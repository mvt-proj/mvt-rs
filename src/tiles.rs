use std::time::Instant;
use std::path::PathBuf;
use anyhow::anyhow;
use bytes::Bytes;
use salvo::http::header::HeaderValue;
use salvo::prelude::*;
use sqlx::PgPool;

enum Via {
    DATABASE,
    DISK,
    REDIS,
}

use crate::{
    cache::DiskCache,
    rediscache::RedisCache,
    catalog::{Catalog, Layer, StateLayer},
    get_catalog, get_db_pool, get_disk_cache,
    get_app_state,
};


fn convert_fields(fields: Vec<String>) -> String {
    let vec_fields: Vec<String>;
    if fields.len() == 1 {
        vec_fields = fields[0]
            .split(',')
            .map(|s| format!("\"{}\"", s.trim()))
            .collect();
    } else {
        vec_fields = fields.iter()
            .map(|field| format!("\"{}\"", field))
            .collect::<Vec<_>>();
    }
    vec_fields.join(", ")
}

async fn query_database(
    pg_pool: PgPool,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    query: String,
) -> Result<Bytes, anyhow::Error> {
    let name = layer_conf.name;
    let schema = layer_conf.schema;
    let table = layer_conf.table;
    let fields = convert_fields(layer_conf.fields);

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
              FROM "{schema}"."{table}"
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
              FROM "{schema}"."{table}"
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

use std::borrow::Cow;

async fn get_tile(
    pg_pool: PgPool,
    disk_cache: DiskCache,
    layer_conf: Layer,
    x: u32,
    y: u32,
    z: u32,
    filter: String,
) -> Result<(Bytes, Via), anyhow::Error> {
    let name = &layer_conf.name;
    let max_cache_age = layer_conf.max_cache_age.unwrap_or(0);

    let query: Cow<str> = if !filter.is_empty() {
        Cow::Borrowed(&filter)
    } else {
        Cow::Owned(layer_conf.clone().filter.unwrap_or_default())
    };

    let tilefolder = disk_cache.cache_dir.join(name).join(&z.to_string()).join(&x.to_string());
    let tilepath = tilefolder.join(&y.to_string()).with_extension("pbf");

    let key = format!("{name}:{z}:{x}:{y}");
    let app_state = get_app_state();
    let use_redis_cache = app_state.use_redis_cache;
    let redis_cache = &app_state.redis_cache;

    if use_redis_cache {
        if let Some(rc) = redis_cache {
            if rc.exists_key(key.clone()).await? {
                let tile = rc.get_cache(key).await?;
                return Ok((tile.into(), Via::REDIS));
            }
        }
    } else if let Ok(cached_tile) = disk_cache.get_cache(tilepath.clone(), max_cache_age).await {
        return Ok((cached_tile, Via::DISK));
    }

    let tile: Bytes = query_database(pg_pool.clone(),
                                     layer_conf.clone(),
                                     x,
                                     y,
                                     z,
                                     query.to_string()
                                     )
        .await?
        .into();

    if write_cache(key, &tile, &tilepath, use_redis_cache, redis_cache, disk_cache, max_cache_age).await.is_ok() {
        Ok((tile.into(), Via::DATABASE))
    } else {
        Err(anyhow!("Error writing cache"))
    }
}

async fn write_cache(
    key: String,
    tile: &Bytes,
    tilepath: &PathBuf,
    use_redis_cache: bool,
    redis_cache: &Option<RedisCache>,
    disk_cache: DiskCache,
    max_cache_age: u64,
) -> Result<(), anyhow::Error> {
    if use_redis_cache {
        if let Some(rc) = redis_cache {
            rc.write_tile_to_cache(key, &tile.to_vec(), max_cache_age).await?;
        }
    } else {
        disk_cache.write_tile_to_file(tilepath, tile).await?;
    }
    Ok(())
}


// async fn get_tile(
//     pg_pool: PgPool,
//     disk_cache: DiskCache,
//     layer_conf: Layer,
//     x: u32,
//     y: u32,
//     z: u32,
//     filter: String,
// ) -> Result<(Bytes, Via), anyhow::Error> {
//     let name = &layer_conf.name;
//     let max_cache_age = layer_conf.max_cache_age.unwrap_or(0);
//
//     // let write_cache = filter.is_empty();
//     let query: String = if !filter.is_empty() {
//         filter
//     } else {
//         layer_conf.clone().filter.unwrap_or(String::new())
//     };
//     let write_cache = query.is_empty();
//
//     let tilefolder = disk_cache
//         .cache_dir
//         .join(name.to_string())
//         .join(&z.to_string())
//         .join(&x.to_string());
//     let tilepath = tilefolder.join(&y.to_string()).with_extension("pbf");
//
//     let key = format!("{name}:{z}:{x}:{y}");
//     let app_state = get_app_state();
//     let use_redis_cache = app_state.use_redis_cache;
//     let redis_cache = &app_state.redis_cache;
//
//     if use_redis_cache {
//         match redis_cache {
//             Some(rc) => {
//                 if rc.exists_key(key.clone()).await? {
//                     let tile  = rc.get_cache(key).await?;
//                     return Ok((tile.into(), Via::REDIS));
//                 }
//             },
//             None => {
//
//             }
//         }
//     } else {
//         if let Ok(cached_tile) = disk_cache.get_cache(tilepath.clone(), max_cache_age).await {
//             return Ok((cached_tile, Via::DISK));
//         }
//     }
//
//     let tile: Bytes = query_database(pg_pool.clone(), layer_conf.clone(), x, y, z, query)
//         .await?
//         .into();
//
//     if write_cache {
//         if use_redis_cache {
//             match redis_cache {
//                 Some(rc) => {
//                     rc.write_tile_to_cache(key, &tile.to_vec(), max_cache_age).await?;
//                 },
//                 None => {
//                     disk_cache.write_tile_to_file(&tilepath, &tile).await?;
//                 }
//             }
//         } else {
//             disk_cache.write_tile_to_file(&tilepath, &tile).await?;
//         }
//     }
//     Ok((tile.into(), Via::DATABASE))
// }

#[handler]
pub async fn mvt(req: &mut Request, res: &mut Response) -> Result<(), anyhow::Error> {
    let layer_name = req.param::<String>("layer_name").unwrap_or("".to_string());
    let x = req.param::<u32>("x").unwrap_or(0);
    let y = req.param::<u32>("y").unwrap_or(0);
    let z = req.param::<u32>("z").unwrap_or(0);
    let filter = req.query::<String>("filter").unwrap_or(String::from(""));

    let pg_pool: PgPool = get_db_pool().clone();
    let catalog: Catalog = get_catalog().clone();
    let disk_cache: DiskCache = get_disk_cache().clone();

    let layer = catalog.find_layer_by_name(&layer_name, StateLayer::PUBLISHED);
    res.headers_mut().insert(
        "content-type",
        "application/x-protobuf;type=mapbox-vector".parse().unwrap(),
    );

    match layer {
        Some(lyr) => {
            let zmin = lyr.zmin.unwrap_or(0);
            let zmax = lyr.zmax.unwrap_or(22);
            if z < zmin || z > zmax {
                res.body(salvo::http::ResBody::Once(Bytes::new()));
                return Ok(());
            }
            let start_time = Instant::now();
            let (tile, via) = get_tile(pg_pool, disk_cache, lyr.clone(), x, y, z, filter).await?;
            let elapsed_time = start_time.elapsed();
            let elapsed_time_str = format!("{}ms - {}us",
                                                   elapsed_time.as_millis(),
                                                   elapsed_time.subsec_micros()
                                                   );
            res.headers_mut()
                .insert("X-Response-Time", HeaderValue::from_str(&elapsed_time_str).unwrap_or_else(|_| HeaderValue::from_static("0")));

            match via {
                Via::DATABASE => {
                    res.headers_mut()
                        .insert("X-Cache", HeaderValue::from_static("MISS"));
                },
                Via::DISK => {
                    res.headers_mut()
                        .insert("X-Cache", HeaderValue::from_static("HIT Cached Disk"));
                },
                Via::REDIS => {
                    res.headers_mut()
                        .insert("X-Cache", HeaderValue::from_static("HIT Cached Redis"));
                }
            }

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
