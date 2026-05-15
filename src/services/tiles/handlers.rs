use bytes::Bytes;
use salvo::http::{StatusCode, header::HeaderValue};
use salvo::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use tracing::warn;

use super::builder::{Via, get_tile};
use crate::services::utils::{get_request_user, validate_user_groups};
use crate::{
    error::{AppError, AppResult},
    filters,
    get_cache_wrapper,
    get_catalog,
    get_db_registry,
    get_plugin_registry,
    models::catalog::StateLayer,
    monitor::record_latency,
};

/// FNV-1a 64-bit hash of an arbitrary string. Used to produce ETags from
/// structured inputs (layer_name:z:x:y:version) without touching tile bytes.
fn compute_etag(input: &str) -> String {
    let mut hash: u64 = 14695981039346656037;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("\"{hash:x}\"")
}

/// Cache-Control value based on the layer's max_cache_age.
/// 0 (infinite server cache) → 24h client cache.
/// >0 → map directly to max-age.
fn cache_control(max_cache_age: u64) -> String {
    if max_cache_age == 0 {
        "public, max-age=3600".to_string()
    } else {
        format!("public, max-age={max_cache_age}")
    }
}

/// Returns true if the client already has the current version (ETag match).
fn is_not_modified(req: &Request, etag: &str) -> bool {
    req.headers()
        .get("if-none-match")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == etag)
        .unwrap_or(false)
}

fn set_cache_headers(res: &mut Response, etag: &str, max_cache_age: u64) {
    if let Ok(v) = HeaderValue::from_str(etag) {
        res.headers_mut().insert("ETag", v);
    }
    if let Ok(v) = HeaderValue::from_str(&cache_control(max_cache_age)) {
        res.headers_mut().insert("Cache-Control", v);
    }
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

    let known_params = ["layer_name", "x", "y", "z"];
    let mut filter_params: HashMap<String, String> = HashMap::new();
    for (key, values) in req.queries() {
        if !known_params.contains(&key.as_str())
            && let Some(value) = values.first()
        {
            filter_params.insert(key.to_string(), value.to_string());
        }
    }
    let has_filters = !filter_params.is_empty();

    let filters = filters::parse_query_params(&filter_params);
    let mut builder = filters::SqlQueryBuilder::new(9);
    let (where_clause, bindings) = builder.build(&filters);

    let layer = {
        let catalog = get_catalog().await.read().await;
        catalog
            .find_layer_by_category_and_name(category, name, StateLayer::Published)
            .cloned()
    };

    let Some(layer) = layer else {
        warn!(category = %category, name = %name, "Layer not found");
        res.status_code(StatusCode::NOT_FOUND);
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    };

    let pg_pool = get_db_registry()
        .get_pool(&layer.database_id)
        .cloned()
        .ok_or_else(|| {
            warn!(db = %layer.database_id, "Database pool not found");
            AppError::DatabaseError("Pool not found".to_string())
        })?;

    let (req_user, req_groups) = get_request_user(req, depot).await;

    if !validate_user_groups(req, &layer, depot).await? {
        warn!(category = %category, name = %name, "User not authorized for layer");
        res.status_code(StatusCode::FORBIDDEN);
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    };

    let zmin = layer.zmin.unwrap_or(0);
    let zmax = layer.zmax.unwrap_or(22);
    if z < zmin || z > zmax {
        warn!(
            category = %category,
            name = %name,
            z = %z,
            zmin = %zmin,
            zmax = %zmax,
            "Zoom level out of range"
        );
        res.status_code(StatusCode::BAD_REQUEST);
        res.body(salvo::http::ResBody::Once(Bytes::new()));
        return Ok(());
    }

    let max_cache_age = layer.max_cache_age.unwrap_or(0);
    let layer_key = format!("{}_{}", layer.category.name, layer.name);
    let has_plugin = get_plugin_registry().has_plugin(&layer_key, category);

    // Filtered or plugin-driven requests are dynamic: skip ETags and client cache.
    // Plugin layers can change per request (time, user) independently of tile version.
    if !has_filters && !has_plugin {
        let version = get_cache_wrapper().get_layer_version(&layer_key).await;
        let etag = compute_etag(&format!("{layer_key}:{z}:{x}:{y}:{version}"));

        // Early exit: browser already has the current version.
        // No DB query, no cache read.
        if is_not_modified(req, &etag) {
            set_cache_headers(res, &etag, max_cache_age);
            res.status_code(StatusCode::NOT_MODIFIED);
            return Ok(());
        }

        let start_time = Instant::now();

        let (tile, via) =
            match get_tile(pg_pool, layer.clone(), x, y, z, where_clause, bindings, req_user.clone(), req_groups.clone()).await {
                Ok(result) => result,
                Err(e) => {
                    res.status_code(StatusCode::BAD_REQUEST);
                    res.render(Json(serde_json::json!({
                        "error": "Invalid filter",
                        "message": e.to_string()
                    })));
                    return Ok(());
                }
            };

        let elapsed_time = start_time.elapsed();
        record_latency(elapsed_time.as_secs_f64());

        res.headers_mut().insert(
            "X-Data-Source-Time",
            HeaderValue::from_str(&elapsed_time.as_millis().to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
        res.headers_mut().insert(
            "X-Cache",
            match via {
                Via::Database => HeaderValue::from_static("MISS"),
                Via::Cache => HeaderValue::from_static("HIT"),
            },
        );

        set_cache_headers(res, &etag, max_cache_age);
        res.body(salvo::http::ResBody::Once(tile));
    } else {
        // Filtered or plugin-driven request: always hits the DB, no server cache, no ETag.
        let start_time = Instant::now();

        let (tile, _) =
            match get_tile(pg_pool, layer.clone(), x, y, z, where_clause, bindings, req_user, req_groups).await {
                Ok(result) => result,
                Err(e) => {
                    res.status_code(StatusCode::BAD_REQUEST);
                    res.render(Json(serde_json::json!({
                        "error": "Invalid filter",
                        "message": e.to_string()
                    })));
                    return Ok(());
                }
            };

        let elapsed_time = start_time.elapsed();
        record_latency(elapsed_time.as_secs_f64());

        res.headers_mut().insert(
            "X-Data-Source-Time",
            HeaderValue::from_str(&elapsed_time.as_millis().to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
        res.headers_mut().insert("X-Cache", HeaderValue::from_static("BYPASS"));

        if has_plugin {
            if let Ok(v) = HeaderValue::from_str("no-store, no-cache") {
                res.headers_mut().insert("Cache-Control", v);
            }
        }

        res.body(salvo::http::ResBody::Once(tile));
    }

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

    let (req_user, req_groups) = get_request_user(req, depot).await;

    let candidates: Vec<_> = {
        let catalog = get_catalog().await.read().await;
        layers_vec
            .iter()
            .filter_map(|layer_name| {
                let (category, name) = layer_name.split_once(':').unwrap_or(("", ""));
                catalog
                    .find_layer_by_category_and_name(category, name, StateLayer::Published)
                    .cloned()
            })
            .collect()
    };

    let mut layer_configs = Vec::new();
    for layer in candidates {
        if validate_user_groups(req, &layer, depot).await? {
            let zmin = layer.zmin.unwrap_or(0);
            let zmax = layer.zmax.unwrap_or(22);
            if z >= zmin && z <= zmax {
                layer_configs.push(layer);
            }
        }
    }

    // Use the most restrictive (smallest) max_cache_age across all layers.
    let min_cache_age = layer_configs
        .iter()
        .map(|l| l.max_cache_age.unwrap_or(0))
        .filter(|&v| v > 0)
        .min()
        .unwrap_or(0);

    let any_has_plugin = layer_configs.iter().any(|l| {
        let key = format!("{}_{}", l.category.name, l.name);
        get_plugin_registry().has_plugin(&key, &l.category.name)
    });

    // Build version-based ETag from all layer versions combined.
    // Skipped when any layer has a plugin (dynamic content).
    let cache_wrapper = get_cache_wrapper();
    let mut etag_input = format!("{z}:{x}:{y}");
    for layer in &layer_configs {
        let key = format!("{}_{}", layer.category.name, layer.name);
        let version = cache_wrapper.get_layer_version(&key).await;
        etag_input.push(':');
        etag_input.push_str(&key);
        etag_input.push(':');
        etag_input.push_str(&version.to_string());
    }
    let etag = compute_etag(&etag_input);

    if !any_has_plugin && is_not_modified(req, &etag) {
        set_cache_headers(res, &etag, min_cache_age);
        res.status_code(StatusCode::NOT_MODIFIED);
        return Ok(());
    }

    let mut futures = Vec::new();
    for layer in layer_configs {
        let pg_pool = match get_db_registry().get_pool(&layer.database_id) {
            Some(pool) => pool.clone(),
            None => continue,
        };
        futures.push(get_tile(pg_pool, layer, x, y, z, String::new(), Vec::new(), req_user.clone(), req_groups.clone()));
    }

    let results = futures::future::join_all(futures).await;

    let mut output_data = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for result in results {
        if let Ok((tile, via)) = result {
            match via {
                Via::Database => cache_misses += 1,
                Via::Cache => cache_hits += 1,
            }
            output_data.push(tile);
        }
    }

    let final_output = Bytes::from(output_data.concat());

    res.headers_mut().insert(
        "X-Cache",
        HeaderValue::from_str(&format!("HIT: {cache_hits}, MISS: {cache_misses}"))
            .unwrap_or_else(|_| HeaderValue::from_static("UNKNOWN")),
    );

    if any_has_plugin {
        if let Ok(v) = HeaderValue::from_str("no-store, no-cache") {
            res.headers_mut().insert("Cache-Control", v);
        }
    } else {
        set_cache_headers(res, &etag, min_cache_age);
    }
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

    let (req_user, req_groups) = get_request_user(req, depot).await;

    let candidates: Vec<_> = {
        let catalog = get_catalog().await.read().await;
        catalog
            .find_layers_by_category(&category, StateLayer::Published)
            .into_iter()
            .cloned()
            .collect()
    };

    let mut layer_configs = Vec::new();
    for layer in candidates {
        if validate_user_groups(req, &layer, depot).await? {
            let zmin = layer.zmin.unwrap_or(0);
            let zmax = layer.zmax.unwrap_or(22);
            if z >= zmin && z <= zmax {
                layer_configs.push(layer);
            }
        }
    }

    let min_cache_age = layer_configs
        .iter()
        .map(|l| l.max_cache_age.unwrap_or(0))
        .filter(|&v| v > 0)
        .min()
        .unwrap_or(0);

    let any_has_plugin = layer_configs.iter().any(|l| {
        let key = format!("{}_{}", l.category.name, l.name);
        get_plugin_registry().has_plugin(&key, &l.category.name)
    });

    // Build version-based ETag from all layer versions combined.
    // Skipped when any layer has a plugin (dynamic content).
    let cache_wrapper = get_cache_wrapper();
    let mut etag_input = format!("{z}:{x}:{y}");
    for layer in &layer_configs {
        let key = format!("{}_{}", layer.category.name, layer.name);
        let version = cache_wrapper.get_layer_version(&key).await;
        etag_input.push(':');
        etag_input.push_str(&key);
        etag_input.push(':');
        etag_input.push_str(&version.to_string());
    }
    let etag = compute_etag(&etag_input);

    if !any_has_plugin && is_not_modified(req, &etag) {
        set_cache_headers(res, &etag, min_cache_age);
        res.status_code(StatusCode::NOT_MODIFIED);
        return Ok(());
    }

    let mut futures = Vec::new();
    for layer in layer_configs {
        let pg_pool = match get_db_registry().get_pool(&layer.database_id) {
            Some(pool) => pool.clone(),
            None => continue,
        };
        futures.push(get_tile(pg_pool, layer, x, y, z, String::new(), Vec::new(), req_user.clone(), req_groups.clone()));
    }

    let results = futures::future::join_all(futures).await;

    let mut output_data = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for result in results {
        if let Ok((tile, via)) = result {
            match via {
                Via::Database => cache_misses += 1,
                Via::Cache => cache_hits += 1,
            }
            output_data.push(tile);
        }
    }

    let final_output = Bytes::from(output_data.concat());

    res.headers_mut().insert(
        "X-Cache",
        HeaderValue::from_str(&format!("HIT: {cache_hits}, MISS: {cache_misses}"))
            .unwrap_or_else(|_| HeaderValue::from_static("UNKNOWN")),
    );

    if any_has_plugin {
        if let Ok(v) = HeaderValue::from_str("no-store, no-cache") {
            res.headers_mut().insert("Cache-Control", v);
        }
    } else {
        set_cache_headers(res, &etag, min_cache_age);
    }
    res.body(salvo::http::ResBody::Once(final_output));
    Ok(())
}
