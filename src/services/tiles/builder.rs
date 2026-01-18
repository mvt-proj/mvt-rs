use bytes::Bytes;
use sqlx::PgPool;

use crate::services::utils::{convert_fields, validate_filter};

use crate::{
    config::consts::*,
    error::AppResult,
    get_cache_wrapper,
    models::catalog::Layer,
    monitor::{record_request, record_cache_hit, record_cache_miss},
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
        if validate_filter(&where_clause).is_err() {
            return Ok(Bytes::new());
        }
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
    let sql_query = sql_template
        .replace("{fields}", &fields)
        .replace("{schema}", &schema)
        .replace("{table}", &table)
        .replace("{geom}", &geom)
        .replace(
            "{query_placeholder}",
            query_placeholder.as_deref().unwrap_or(""),
        )
        .replace("{limit_placeholder}", &limit_clause);

    let mut query_builder = sqlx::query_as::<_, (Option<Vec<u8>>,)>(&sql_query)
        .bind(z as i32)
        .bind(x as i32)
        .bind(y as i32)
        .bind(extent as i32)
        .bind(buffer as i32)
        .bind(clip_geom)
        .bind(srid as i32)
        .bind(name);

    if !where_clause.is_empty() {
        for binding in bindings {
            if let Ok(num) = binding.parse::<i64>() {
                query_builder = query_builder.bind(num);
            } else if let Ok(num) = binding.parse::<f64>() {
                query_builder = query_builder.bind(num);
            } else {
                query_builder = query_builder.bind(binding);
            }
        }
    }

    let rec = query_builder.fetch_one(&pg_pool).await?;
    let tile = rec.0.unwrap_or_default();

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
) -> AppResult<(Bytes, Via)> {
    // let name = &layer_conf.name;
    let name_owned = format!("{}_{}", layer_conf.category.name, layer_conf.name);
    let name = &name_owned;
    let max_cache_age = layer_conf.max_cache_age.unwrap_or(0);
    let mut local_where_clause = where_clause;
    let original_local_where_clause_is_empty = local_where_clause.is_empty();

    let query = layer_conf.clone().filter.unwrap_or_default();
    let cache_wrapper = get_cache_wrapper();

    record_request();

    if local_where_clause.is_empty()
        && let Some(tile) = cache_wrapper.get_tile(name, z, x, y, max_cache_age).await
    {
        record_cache_hit();
        return Ok((tile, Via::Cache));
    }
    record_cache_miss();

    if !query.is_empty() {
        if validate_filter(&query).is_err() {
            return Ok((Bytes::new(), Via::Database));
        }
        if !local_where_clause.is_empty() {
            local_where_clause.push_str(" AND ");
        }
        local_where_clause.push_str(&query);
    }

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

    if original_local_where_clause_is_empty {
        cache_wrapper
            .write_tile(name, z, x, y, &tile, max_cache_age)
            .await?;
    }

    Ok((tile, Via::Database))
}
