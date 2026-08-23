use bytes::Bytes;
use sqlx::PgPool;
// use tracing::{error, warn};

use crate::services::utils::{convert_fields, validate_filter};
use crate::{
    config::consts::*,
    error::AppResult,
    get_cache_wrapper,
    get_plugin_registry,
    models::catalog::Layer,
    monitor::{record_cache_hit, record_cache_miss, record_request},
    plugins::PluginContext,
};

pub enum Via {
    Database,
    Cache,
}

pub fn build_sql_template(sql_mode: &str) -> &'static str {
    match sql_mode {
        "CTE" => {
            r#"
            WITH mvtgeom AS (
                SELECT
                    {fields},
                    ST_AsMVTGeom(
                        ST_Transform({geom}, 3857),
                        ST_TileEnvelope($1, $2, $3),
                        $4, $5, $6
                    ) AS geom
                FROM "{schema}"."{table}"
                WHERE {geom} && ST_Transform(ST_TileEnvelope($1, $2, $3), $7)
                    AND {geom} IS NOT NULL
                    {query_placeholder}
                {limit_placeholder}
            )
            SELECT ST_AsMVT(mvtgeom.*, $8, $4, 'geom') AS tile FROM mvtgeom;
        "#
        }
        _ => {
            r#"
            SELECT ST_AsMVT(tile, $8, $4, 'geom') FROM (
                SELECT
                    {fields},
                    ST_AsMVTGeom(
                        ST_Transform({geom}, 3857),
                        ST_TileEnvelope($1, $2, $3),
                        $4, $5, $6
                    ) AS geom
                FROM "{schema}"."{table}"
                WHERE {geom} && ST_Transform(ST_TileEnvelope($1, $2, $3), $7)
                    AND {geom} IS NOT NULL
                    {query_placeholder}
                {limit_placeholder}
            ) as tile;
        "#
        }
    }
}

/// Point on which a `_labels` MVT layer can anchor a symbol, so a feature
/// that spans several tiles only gets one label instead of one per fragment.
/// `None` for "points" geometries, where a label sub-layer wouldn't add anything.
fn label_geometry_expr(geometry: &str, geom: &str) -> Option<String> {
    match geometry {
        "polygons" | "lines" => Some(format!("ST_PointOnSurface({geom})")),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_layer_query(
    pg_pool: &PgPool,
    sql_template: &str,
    fields: &str,
    schema: &str,
    table: &str,
    geom_expr: &str,
    query_placeholder: Option<&str>,
    limit_clause: &str,
    x: u32,
    y: u32,
    z: u32,
    extent: u32,
    buffer: u32,
    clip_geom: bool,
    srid: u32,
    mvt_layer_name: &str,
    where_clause: &str,
    bindings: &[String],
) -> AppResult<Vec<u8>> {
    let sql_query = sql_template
        .replace("{fields}", fields)
        .replace("{schema}", schema)
        .replace("{table}", table)
        .replace("{geom}", geom_expr)
        .replace("{query_placeholder}", query_placeholder.unwrap_or(""))
        .replace("{limit_placeholder}", limit_clause);

    let mut query_builder = sqlx::query_as::<_, (Option<Vec<u8>>,)>(sqlx::AssertSqlSafe(sql_query))
        .bind(z as i32)
        .bind(x as i32)
        .bind(y as i32)
        .bind(extent as i32)
        .bind(buffer as i32)
        .bind(clip_geom)
        .bind(srid as i32)
        .bind(mvt_layer_name.to_string());

    if !where_clause.is_empty() {
        for binding in bindings {
            if let Ok(num) = binding.parse::<i64>() {
                query_builder = query_builder.bind(num);
            } else if let Ok(num) = binding.parse::<f64>() {
                query_builder = query_builder.bind(num);
            } else {
                query_builder = query_builder.bind(binding.clone());
            }
        }
    }

    let rec = query_builder.fetch_one(pg_pool).await?;
    Ok(rec.0.unwrap_or_default())
}

pub async fn query_database(
    pg_pool: PgPool,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    where_clause: String,
    bindings: Vec<String>,
) -> AppResult<Bytes> {
    let name = layer_conf.name;
    let schema = layer_conf.schema;
    let table = layer_conf.table_name;
    let fields = convert_fields(layer_conf.fields);
    let geom = layer_conf.geom.unwrap_or_else(|| "geom".to_string());
    let sql_mode = layer_conf.sql_mode.unwrap_or_else(|| "CTE".to_string());
    let srid = layer_conf.srid.unwrap_or(DEFAULT_SRID);

    let query_placeholder = if !where_clause.is_empty() {
        validate_filter(&where_clause)?;
        Some(format!(" AND {where_clause}"))
    } else {
        None
    };

    let (buffer, extent) = if z
        >= layer_conf
            .zmax_do_not_simplify
            .unwrap_or(DEFAULT_ZMAX_DO_NOT_SIMPLIFY)
    {
        (
            layer_conf.buffer_do_not_simplify.unwrap_or(DEFAULT_BUFFER),
            layer_conf.extent_do_not_simplify.unwrap_or(DEFAULT_EXTENT),
        )
    } else {
        (
            layer_conf.buffer.unwrap_or(DEFAULT_BUFFER),
            layer_conf.extent.unwrap_or(DEFAULT_EXTENT),
        )
    };

    let clip_geom = layer_conf.clip_geom.unwrap_or(true);

    let limit_clause = layer_conf
        .max_records
        .filter(|&max| max > 0)
        .map_or_else(String::new, |max| format!("ORDER BY RANDOM() LIMIT {max}"));

    let sql_template = build_sql_template(&sql_mode);

    let main_query = run_layer_query(
        &pg_pool,
        sql_template,
        &fields,
        &schema,
        &table,
        &geom,
        query_placeholder.as_deref(),
        &limit_clause,
        x,
        y,
        z,
        extent,
        buffer,
        clip_geom,
        srid,
        &name,
        &where_clause,
        &bindings,
    );

    let label_expr = layer_conf
        .label_layer
        .then(|| label_geometry_expr(&layer_conf.geometry, &geom))
        .flatten();

    let tile = match &label_expr {
        Some(expr) => {
            let label_name = format!("{name}_labels");
            let label_query = run_layer_query(
                &pg_pool,
                sql_template,
                &fields,
                &schema,
                &table,
                expr,
                query_placeholder.as_deref(),
                &limit_clause,
                x,
                y,
                z,
                extent,
                buffer,
                clip_geom,
                srid,
                &label_name,
                &where_clause,
                &bindings,
            );
            let (mut main_tile, label_tile) = tokio::try_join!(main_query, label_query)?;
            main_tile.extend_from_slice(&label_tile);
            main_tile
        }
        None => main_query.await?,
    };

    Ok(tile.into())
}

pub async fn get_tile(
    pg_pool: PgPool,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    where_clause: String,
    bindings: Vec<String>,
    user: Option<String>,
    groups: Option<Vec<String>>,
) -> AppResult<(Bytes, Via)> {
    let name_owned = format!("{}_{}", layer_conf.category.name, layer_conf.name);
    let name = &name_owned;
    let max_cache_age = layer_conf.max_cache_age.unwrap_or(0);
    let mut local_where_clause = where_clause;
    let original_local_where_clause_is_empty = local_where_clause.is_empty();

    let query = layer_conf.clone().filter.unwrap_or_default();
    let cache_wrapper = get_cache_wrapper();

    record_request();

    // Layers with an active Lua plugin bypass server cache: the plugin may
    // produce different filters depending on context (future: user, time, etc.)
    let category = &layer_conf.category.name;
    let has_plugin = get_plugin_registry().has_plugin(name, category);

    if local_where_clause.is_empty() && !has_plugin
        && let Some(tile) = cache_wrapper.get_tile(name, z, x, y, max_cache_age).await
    {
        record_cache_hit();
        return Ok((tile, Via::Cache));
    }
    record_cache_miss();

    if !query.is_empty() {
        validate_filter(&query)?;
        if !local_where_clause.is_empty() {
            local_where_clause.push_str(" AND ");
        }
        local_where_clause.push_str(&query);
    }

    // --- Lua plugin: filter hook ---
    let ctx = PluginContext {
        layer: layer_conf.name.clone(),
        category: category.clone(),
        z,
        x,
        y,
        user,
        groups,
    };
    if let Some(lua_filter) = get_plugin_registry().call_filter(name, category, &ctx).await {
        if !lua_filter.is_empty() {
            validate_filter(&lua_filter)?;
            if !local_where_clause.is_empty() {
                local_where_clause.push_str(" AND ");
            }
            local_where_clause.push_str(&lua_filter);
        }
    }
    // --- end Lua plugin ---

    let tile: Bytes = query_database(
        pg_pool.clone(),
        layer_conf.clone(),
        x,
        y,
        z,
        local_where_clause.clone(),
        bindings,
    )
    .await?;

    if original_local_where_clause_is_empty && !has_plugin {
        cache_wrapper
            .write_tile(name, z, x, y, &tile, max_cache_age)
            .await?;
    }

    Ok((tile, Via::Database))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_geometry_expr_polygons_and_lines_use_point_on_surface() {
        assert_eq!(
            label_geometry_expr("polygons", "geom"),
            Some("ST_PointOnSurface(geom)".to_string())
        );
        assert_eq!(
            label_geometry_expr("lines", "the_geom"),
            Some("ST_PointOnSurface(the_geom)".to_string())
        );
    }

    #[test]
    fn label_geometry_expr_none_for_points() {
        assert_eq!(label_geometry_expr("points", "geom"), None);
    }
}
