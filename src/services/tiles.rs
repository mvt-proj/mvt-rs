use bytes::Bytes;
use salvo::http::header::HeaderValue;
use salvo::prelude::*;
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Instant;

use crate::{
    error::AppResult,
    get_app_state, get_catalog, get_db_pool,
    html::main::get_session_data,
    models::catalog::{Catalog, Layer, StateLayer},
};

const DEFAULT_BUFFER: u32 = 256;
const DEFAULT_EXTENT: u32 = 4096;
const DEFAULT_SRID: u32 = 4326;
const DEFAULT_ZMAX_DO_NOT_SIMPLIFY: u32 = 16;

enum Via {
    Database,
    Cache,
}

fn convert_fields(fields: Vec<String>) -> String {
    let vec_fields: Vec<String> = if fields.len() == 1 {
        fields[0]
            .split(',')
            .map(|s| format!("\"{}\"", s.trim()))
            .collect()
    } else {
        fields
            .iter()
            .map(|field| format!("\"{}\"", field))
            .collect::<Vec<_>>()
    };
    vec_fields.join(", ")
}

fn build_sql_query(
    sql_mode: &str,
    name: &str,
    schema: &str,
    table: &str,
    fields: &str,
    geom: &str,
    z: u32,
    x: u32,
    y: u32,
    extent: u32,
    buffer: u32,
    clip_geom: &str,
    srid: u32,
    query_placeholder: &str,
) -> String {
    match sql_mode {
        "CTE" => format!(
            r#"
            WITH mvtgeom AS (
                SELECT
                    {fields},
                    ST_AsMVTGeom(
                        ST_Transform({geom}, 3857),
                        ST_TileEnvelope({z}, {x}, {y}),
                        {extent},
                        {buffer},
                        {clip_geom}
                    ) AS geom
                FROM "{schema}"."{table}"
                WHERE
                    {geom} && ST_Transform(ST_TileEnvelope({z}, {x}, {y}), {srid})
                    AND {geom} IS NOT NULL
                    {query_placeholder}
            )
            SELECT ST_AsMVT(mvtgeom.*, '{name}', {extent}, 'geom') AS tile
            FROM mvtgeom;
            "#
        ),
        _ => format!(
            r#"
            SELECT ST_AsMVT(tile, '{name}', {extent}, 'geom') FROM (
                SELECT
                    {fields},
                    ST_AsMVTGeom(
                        ST_Transform({geom}, 3857),
                        ST_TileEnvelope({z}, {x}, {y}),
                        {extent},
                        {buffer},
                        {clip_geom}
                    ) AS geom
                FROM "{schema}"."{table}"
                WHERE
                    {geom} && ST_Transform(ST_TileEnvelope({z}, {x}, {y}), {srid})
                    AND {geom} IS NOT NULL
                    {query_placeholder}
            ) as tile;
            "#
        ),
    }
}

async fn query_database(
    pg_pool: PgPool,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    query: String,
) -> AppResult<Bytes> {
    let name = layer_conf.name;
    let schema = layer_conf.schema;
    let table = layer_conf.table_name;
    let fields = convert_fields(layer_conf.fields);
    let geom = layer_conf.geom.unwrap_or_else(|| "geom".to_string());
    let sql_mode = layer_conf.sql_mode.unwrap_or_else(|| "CTE".to_string());
    let srid = layer_conf.srid.unwrap_or(DEFAULT_SRID);

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

    let clip_geom = layer_conf.clip_geom.unwrap_or(true).to_string();
    let query_placeholder = if !query.is_empty() {
        format!("AND {query}")
    } else {
        String::new()
    };

    let sql = build_sql_query(
        &sql_mode,
        &name,
        &schema,
        &table,
        &fields,
        &geom,
        z,
        x,
        y,
        extent,
        buffer,
        &clip_geom,
        srid,
        &query_placeholder,
    );

    let rec: (Option<Vec<u8>>,) = sqlx::query_as(&sql).fetch_one(&pg_pool).await?;
    let tile = rec.0.unwrap_or_default();
    Ok(tile.into())
}

async fn get_tile(
    pg_pool: PgPool,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    filter: String,
) -> AppResult<(Bytes, Via)> {
    let name = &layer_conf.name;
    let max_cache_age = layer_conf.max_cache_age.unwrap_or(0);

    let query: Cow<str> = if !filter.is_empty() {
        Cow::Borrowed(&filter)
    } else {
        Cow::Owned(layer_conf.clone().filter.unwrap_or_default())
    };

    let app_state = get_app_state();
    let cache_wrapper = &app_state.cache_wrapper;

    if let Ok(tile) = cache_wrapper.get_cache(name, x, y, z, max_cache_age).await {
        return Ok((tile, Via::Cache));
    }

    let tile: Bytes = query_database(
        pg_pool.clone(),
        layer_conf.clone(),
        x,
        y,
        z,
        query.to_string(),
    )
    .await?;

    cache_wrapper
        .write_tile_to_cache(name, x, y, z, &tile, max_cache_age)
        .await?;

    Ok((tile, Via::Database))
}

async fn validate_user_groups(req: &Request, layer: &Layer, depot: &mut Depot) -> AppResult<bool> {
    let Some(groups) = layer.groups.as_ref() else {
        return Ok(true);
    };

    if groups.is_empty() {
        return Ok(true);
    }

    let authorization = req
        .headers()
        .get("authorization")
        .and_then(|ah| ah.to_str().ok())
        .unwrap_or("");

    let user = if !authorization.is_empty() {
        let app_state = crate::get_app_state();
        app_state
            .auth
            .get_user_by_authorization(authorization)?
            .cloned()
    } else {
        None
    };

    let has_common_group = user.as_ref().map_or(false, |user| {
        let user_group_ids: HashSet<_> = user.groups.iter().map(|g| &g.id).collect();
        groups.iter().any(|g| user_group_ids.contains(&g.id))
    });

    let (is_auth, _) = get_session_data(depot);
    Ok(has_common_group || is_auth)
}

#[handler]
pub async fn mvt(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    res.headers_mut().insert(
        "content-type",
        "application/x-protobuf;type=mapbox-vector".parse()?,
    );

    let layer_name = req.param::<String>("layer_name").unwrap_or_default();
    let (category, name) = layer_name.split_once(':').unwrap_or(("", ""));
    let x = req.param::<u32>("x").unwrap_or(0);
    let y = req.param::<u32>("y").unwrap_or(0);
    let z = req.param::<u32>("z").unwrap_or(0);
    let filter = req.query::<String>("filter").unwrap_or_default();

    let pg_pool: PgPool = get_db_pool().clone();
    let catalog: Catalog = get_catalog().clone();

    let Some(layer) =
        catalog.find_layer_by_category_and_name(category, name, StateLayer::Published)
    else {
        tracing::warn!("the layer {}:{} is not found", category, name);
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    };

    if !validate_user_groups(&req, &layer, depot).await? {
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    }

    let zmin = layer.zmin.unwrap_or(0);
    let zmax = layer.zmax.unwrap_or(22);
    if z < zmin || z > zmax {
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    }

    let start_time = Instant::now();
    let (tile, via) = get_tile(pg_pool, layer.clone(), x, y, z, filter).await?;
    let elapsed_time = start_time.elapsed();
    let elapsed_time_str = format!("{}", elapsed_time.as_millis());

    res.headers_mut().insert(
        "X-Data-Source-Time",
        HeaderValue::from_str(&elapsed_time_str).unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    match via {
        Via::Database => {
            res.headers_mut()
                .insert("X-Cache", HeaderValue::from_static("MISS"));
        }
        Via::Cache => {
            res.headers_mut()
                .insert("X-Cache", HeaderValue::from_static("HIT Cached"));
        }
    }

    res.body(salvo::http::ResBody::Once(tile));
    Ok(())
}
