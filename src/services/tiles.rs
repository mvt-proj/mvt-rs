use bytes::Bytes;
use regex::Regex;
use salvo::http::{header::HeaderValue, HeaderMap};
use salvo::prelude::*;
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Instant;

use crate::{
    error::{AppError, AppResult},
    get_auth, get_cache_wrapper, get_catalog, get_db_pool,
    html::main::get_session_data,
    models::catalog::{Layer, StateLayer},
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

fn is_inside_quotes(filter: &str, pos: usize) -> bool {
    let mut in_quotes = false;
    for (i, c) in filter.chars().enumerate() {
        if c == '\'' {
            in_quotes = !in_quotes;
        }
        if i == pos {
            return in_quotes;
        }
    }
    false
}

fn validate_filter(filter: &str) -> AppResult<()> {
    let dangerous_keywords = [
        "DELETE", "UPDATE", "INSERT", "DROP", "TRUNCATE", "CREATE", "EXEC", "EXECUTE",
    ];

    let pattern = format!(r"(?i)\b(?:{})\b", dangerous_keywords.join("|"));
    let re =
        Regex::new(&pattern).map_err(|e| AppError::InvalidInput(format!("Regex error: {}", e)))?;

    let forbidden_patterns = vec![";", "--", "/*", "*/", "OR 1=1"];
    for pattern in forbidden_patterns {
        if filter.contains(pattern) {
            return Err(AppError::InvalidInput(format!(
                "Invalid filter: contains forbidden pattern '{}'",
                pattern
            )));
        }
    }

    for cap in re.find_iter(filter) {
        if !is_inside_quotes(filter, cap.start()) {
            return Err(AppError::InvalidInput(format!(
                "Invalid filter: contains dangerous keyword '{}'",
                cap.as_str()
            )));
        }
    }

    Ok(())
}

fn build_sql_template(sql_mode: &str) -> &'static str {
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

    let query_placeholder = if !query.is_empty() {
        if validate_filter(&query).is_err() {
            return Ok(Bytes::new());
        }
        Some(format!(" AND {}", query))
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
        .map_or_else(String::new, |max| {
            format!("ORDER BY RANDOM() LIMIT {}", max)
        });

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

    let query_builder = sqlx::query_as::<_, (Option<Vec<u8>>,)>(&sql_query)
        .bind(z as i32)
        .bind(x as i32)
        .bind(y as i32)
        .bind(extent as i32)
        .bind(buffer as i32)
        .bind(clip_geom)
        .bind(srid as i32)
        .bind(name);

    let rec = query_builder.fetch_one(&pg_pool).await?;
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

    let cache_wrapper = get_cache_wrapper();

    if filter.is_empty() {
        if let Ok(tile) = cache_wrapper.get_cache(name, x, y, z, max_cache_age).await {
            return Ok((tile, Via::Cache));
        }
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

    if filter.is_empty() {
        cache_wrapper
            .write_tile_to_cache(name, x, y, z, &tile, max_cache_age)
            .await?;
    }

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
        let mut auth = get_auth().await.write().await;
        auth.get_user_by_authorization(authorization)?.cloned()
    } else {
        None
    };

    let has_common_group = user.as_ref().is_some_and(|user| {
        let user_group_ids: HashSet<_> = user.groups.iter().map(|g| &g.id).collect();
        groups.iter().any(|g| user_group_ids.contains(&g.id))
    });

    let (is_auth, _) = get_session_data(depot).await;
    Ok(has_common_group || is_auth)
}

#[handler]
pub async fn get_single_layer_tile(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
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
    let catalog = get_catalog().await.read().await;

    let Some(layer) =
        catalog.find_layer_by_category_and_name(category, name, StateLayer::Published)
    else {
        tracing::warn!("the layer {}:{} is not found", category, name);
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    };

    if !validate_user_groups(req, layer, depot).await? {
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

#[handler]
pub async fn get_composite_layers_tile(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    res.headers_mut().insert(
        "content-type",
        "application/x-protobuf;type=mapbox-vector".parse()?,
    );

    let layers = req.param::<String>("layers").unwrap_or_default();
    let x = req.param::<u32>("x").unwrap_or(0);
    let y = req.param::<u32>("y").unwrap_or(0);
    let z = req.param::<u32>("z").unwrap_or(0);

    let layers_vec: Vec<String> = layers
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let pg_pool: PgPool = get_db_pool().clone();
    let catalog = get_catalog().await.read().await;

    let mut output_data = Vec::new();
    let mut data_source_times = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for layer_name in layers_vec {
        let filter = String::new();
        let (category, name) = layer_name.split_once(':').unwrap_or(("", ""));
        let Some(layer) =
            catalog.find_layer_by_category_and_name(category, name, StateLayer::Published)
        else {
            tracing::warn!("the layer {}:{} is not found", category, name);
            continue;
        };

        if !validate_user_groups(req, layer, depot).await? {
            continue;
        }

        let zmin = layer.zmin.unwrap_or(0);
        let zmax = layer.zmax.unwrap_or(22);
        if z < zmin || z > zmax {
            continue;
        }

        let start_time = Instant::now();
        let (tile, via) = get_tile(pg_pool.clone(), layer.clone(), x, y, z, filter).await?;
        let elapsed_time = start_time.elapsed().as_millis();

        data_source_times.push(format!("{}: {}ms", layer_name, elapsed_time));

        match via {
            Via::Database => cache_misses += 1,
            Via::Cache => cache_hits += 1,
        }

        output_data.push(tile);
    }

    let mut headers = HeaderMap::new();

    if !data_source_times.is_empty() {
        let times_str = data_source_times.join(", ");
        headers.insert(
            "X-Data-Source-Time",
            HeaderValue::from_str(&times_str).unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
    }

    headers.insert(
        "X-Cache",
        HeaderValue::from_str(&format!("HIT: {}, MISS: {}", cache_hits, cache_misses))
            .unwrap_or_else(|_| HeaderValue::from_static("UNKNOWN")),
    );

    res.headers_mut().extend(headers);

    let final_output = Bytes::from(output_data.concat());
    res.body(salvo::http::ResBody::Once(final_output));

    Ok(())
}

#[handler]
pub async fn get_category_layers_tile(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    res.headers_mut().insert(
        "content-type",
        "application/x-protobuf;type=mapbox-vector".parse()?,
    );

    let category = req.param::<String>("category").unwrap_or_default();
    let x = req.param::<u32>("x").unwrap_or(0);
    let y = req.param::<u32>("y").unwrap_or(0);
    let z = req.param::<u32>("z").unwrap_or(0);

    let pg_pool: PgPool = get_db_pool().clone();
    let catalog = get_catalog().await.read().await;

    let layers_vec = catalog.find_layers_by_category(&category, StateLayer::Published);
    let mut output_data = Vec::new();
    let mut data_source_times = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for layer in layers_vec {
        let filter = String::new();

        if !validate_user_groups(req, layer, depot).await? {
            continue;
        }

        let zmin = layer.zmin.unwrap_or(0);
        let zmax = layer.zmax.unwrap_or(22);
        if z < zmin || z > zmax {
            continue;
        }

        let start_time = Instant::now();
        let (tile, via) = get_tile(pg_pool.clone(), layer.clone(), x, y, z, filter).await?;
        let elapsed_time = start_time.elapsed().as_millis();

        data_source_times.push(format!("{}: {}ms", layer.name, elapsed_time));

        match via {
            Via::Database => cache_misses += 1,
            Via::Cache => cache_hits += 1,
        }

        output_data.push(tile);
    }

    let mut headers = HeaderMap::new();

    if !data_source_times.is_empty() {
        let times_str = data_source_times.join(", ");
        headers.insert(
            "X-Data-Source-Time",
            HeaderValue::from_str(&times_str).unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
    }

    headers.insert(
        "X-Cache",
        HeaderValue::from_str(&format!("HIT: {}, MISS: {}", cache_hits, cache_misses))
            .unwrap_or_else(|_| HeaderValue::from_static("UNKNOWN")),
    );

    res.headers_mut().extend(headers);

    let final_output = Bytes::from(output_data.concat());
    res.body(salvo::http::ResBody::Once(final_output));

    Ok(())
}
