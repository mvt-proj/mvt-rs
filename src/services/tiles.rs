use bytes::Bytes;
use salvo::http::header::HeaderValue;
use salvo::prelude::*;
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Instant;

enum Via {
    Database,
    Cache,
}

use crate::{
    auth::User,
    error::AppResult,
    get_app_state, get_catalog, get_db_pool,
    html::main::get_session_data,
    models::catalog::{Catalog, Layer, StateLayer},
};

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

    let geom = layer_conf.geom.unwrap_or(String::from("geom"));
    let sql_mode = layer_conf.sql_mode.unwrap_or(String::from("CTE"));
    let srid = layer_conf.srid.unwrap_or(4326);
    let mut buffer = layer_conf.buffer.unwrap_or(256);
    let mut extent = layer_conf.extent.unwrap_or(4096);

    let zmax_do_not_simplify = layer_conf.zmax_do_not_simplify.unwrap_or(16);
    let buffer_do_not_simplify = layer_conf.buffer_do_not_simplify.unwrap_or(256);
    let extent_do_not_simplify = layer_conf.extent_do_not_simplify.unwrap_or(4096);

    if z >= zmax_do_not_simplify {
        buffer = buffer_do_not_simplify;
        extent = extent_do_not_simplify;
    }

    let clip_geom = layer_conf.clip_geom.unwrap_or(true).to_string();

    let query_placeholder = if !query.is_empty() {
        format!("AND {query}")
    } else {
        String::new()
    };

    let sql: String = if sql_mode == "CTE" {
        format!(
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
                    geom && ST_Transform(ST_TileEnvelope({z}, {x}, {y}), {srid})
                    AND {geom} IS NOT NULL
                    {query_placeholder}
            )
            SELECT ST_AsMVT(mvtgeom.*, '{name}', {extent}, 'geom') AS tile
            FROM mvtgeom;
            "#
        )
    } else {
        format!(
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
                    geom && ST_Transform(ST_TileEnvelope({z}, {x}, {y}), {srid})
                    AND {geom} IS NOT NULL
                    {query_placeholder}
                ) as tile;
            "#
        )
    };

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

    let layer = catalog.find_layer_by_category_and_name(category, name, StateLayer::Published);

    if let Some(lyr) = layer {
        // ====================================================================================
        if let Some(lyr_groups) = lyr.groups.as_ref() {
            if !lyr_groups.is_empty() {
                let authorization = req
                    .headers()
                    .get("authorization")
                    .and_then(|ah| ah.to_str().ok())
                    .unwrap_or("");

                let user: Option<User> = if !authorization.is_empty() {
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
                    lyr_groups.iter().any(|g| user_group_ids.contains(&g.id))
                });

                let (is_auth, _user) = get_session_data(depot);
                if !has_common_group && !is_auth {
                    res.body(salvo::http::ResBody::Once(Bytes::new()));
                    return Ok(());
                }
            }
        }
        // ====================================================================================

        let zmin = lyr.zmin.unwrap_or(0);
        let zmax = lyr.zmax.unwrap_or(22);
        if z < zmin || z > zmax {
            res.body(salvo::http::ResBody::Once(Bytes::new()));
            return Ok(());
        }

        let start_time = Instant::now();
        let (tile, via) = get_tile(pg_pool, lyr.clone(), x, y, z, filter).await?;
        let elapsed_time = start_time.elapsed();
        let elapsed_time_str = format!("{}", elapsed_time.as_millis());
        res.headers_mut().insert(
            "X-Data-Source-Time",
            HeaderValue::from_str(&elapsed_time_str)
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
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
    } else {
        tracing::warn!("the layer {}:{} is not found", category, name);
        res.body(salvo::http::ResBody::Once(Bytes::new()));
    }
    Ok(())
}
