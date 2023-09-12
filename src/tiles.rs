use bytes::Bytes;
use salvo::prelude::*;
use sqlx::PgPool;

use std::path::PathBuf;

use crate::{
    cache,
    config::{Layer, LayersConfig},
    Config, CACHE_DIR,
};

async fn query_database(
    pg_pool: PgPool,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    query: String,
) -> Result<Bytes, anyhow::Error> {
    let name = layer_conf.name;
    let schema = layer_conf.schema.unwrap_or(String::from("public"));
    let table = layer_conf.table;
    let fields = layer_conf.fields.join(", ");

    // let query: String = if !filter.is_empty() {
    //     filter
    // } else {
    //     layer_conf.filter.unwrap_or(String::new())
    // };

    let geom = layer_conf.geom.unwrap_or(String::from("geom"));
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

    let clip_geom = if layer_conf.clip_geom.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let sql: String;
    if !query.is_empty() {
        sql = format!(
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
              FROM {schema}.{table}
              WHERE
                geom && ST_Transform(ST_TileEnvelope({z}, {x}, {y}), {srid})
                AND {geom} IS NOT NULL
                AND {query}
            ) as tile;
        "#
        );
    } else {
        sql = format!(
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
              FROM {schema}.{table}
              WHERE
                geom && ST_Transform(ST_TileEnvelope({z}, {x}, {y}), {srid})
                AND {geom} IS NOT NULL
            ) as tile;
        "#
        );
    }

    let rec: (Option<Vec<u8>>,) = sqlx::query_as(&sql).fetch_one(&pg_pool).await.unwrap();

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
) -> Result<Bytes, anyhow::Error> {
    let cache_dir: PathBuf = CACHE_DIR.get().unwrap().into();
    let name = &layer_conf.name;
    let max_cache_age = layer_conf.max_cache_age.unwrap_or(0);

    let write_cache = filter.is_empty();
    let query: String = if !filter.is_empty() {
        filter
    } else {
        layer_conf.clone().filter.unwrap_or(String::new())
    };

    let tilefolder = cache_dir
        .join(name.to_string())
        .join(&z.to_string())
        .join(&x.to_string());
    let tilepath = tilefolder.join(&y.to_string()).with_extension("pbf");

    if let Ok(cached_tile) = cache::get_cache(tilepath.clone(), max_cache_age).await {
        return Ok(cached_tile);
    }

    let tile: Bytes = query_database(pg_pool.clone(), layer_conf.clone(), x, y, z, query)
        .await?
        .into();

    if write_cache {
        cache::write_tile_to_file(&tilepath, &tile).await?;
    }
    Ok(tile.into())
}

#[handler]
pub async fn mvt(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), anyhow::Error> {
    let layer = req.param::<String>("layer").unwrap_or("".to_string());
    let x = req.param::<u32>("x").unwrap_or(0);
    let y = req.param::<u32>("y").unwrap_or(0);
    let z = req.param::<u32>("z").unwrap_or(0);
    let filter = req.query::<String>("filter").unwrap_or(String::from(""));

    let config = depot.obtain::<Config>().unwrap();
    let config = config.clone();
    let pg_pool: PgPool = config.db_pool;
    let layers_config: LayersConfig = config.layers_config;

    let layer_conf = layers_config.find_layer_by_name(&layer);
    res.headers_mut().insert(
        "content-type",
        "application/x-protobuf;type=mapbox-vector".parse().unwrap(),
    );

    match layer_conf {
        Some(lyr) => {
            let zmin = lyr.zmin.unwrap_or(0);
            let zmax = lyr.zmax.unwrap_or(22);
            if z < zmin || z > zmax {
                res.body(salvo::http::ResBody::Once(Bytes::new()));
                return Ok(());
            }
            let tile = get_tile(pg_pool, lyr.clone(), x, y, z, filter).await?;
            res.body(salvo::http::ResBody::Once(tile));
            Ok(())
        }
        None => {
            tracing::warn!("the layer is not found");
            res.body(salvo::http::ResBody::Once(Bytes::new()));
            Ok(())
        }
    }
}
