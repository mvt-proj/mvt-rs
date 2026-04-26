use bytes::Bytes;
use salvo::http::{StatusCode, header::HeaderValue};
use salvo::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use tracing::warn;

use super::builder::{Via, get_tile};
use crate::services::utils::validate_user_groups;
use crate::{
    error::{AppError, AppResult},
    filters,
    get_catalog,
    get_db_registry,
    models::catalog::StateLayer,
    monitor::record_latency,
};

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
    let filters = filters::parse_query_params(&filter_params);
    let mut builder = filters::SqlQueryBuilder::new(9);
    let (where_clause, bindings) = builder.build(&filters);

    let catalog = get_catalog().await.read().await;

    let Some(layer) =
        catalog.find_layer_by_category_and_name(category, name, StateLayer::Published)
    else {
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

    if !validate_user_groups(req, layer, depot).await? {
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

    let start_time = Instant::now();

    let (tile, via) = match get_tile(pg_pool, layer.clone(), x, y, z, where_clause, bindings).await
    {
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
    let elapsed_secs = elapsed_time.as_secs_f64();
    record_latency(elapsed_secs);

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

    let catalog = get_catalog().await.read().await;

    let mut layer_configs = Vec::new();
    for layer_name in layers_vec {
        let (category, name) = layer_name.split_once(':').unwrap_or(("", ""));
        if let Some(layer) = catalog.find_layer_by_category_and_name(category, name, StateLayer::Published) {
            if validate_user_groups(req, layer, depot).await? {
                let zmin = layer.zmin.unwrap_or(0);
                let zmax = layer.zmax.unwrap_or(22);
                if z >= zmin && z <= zmax {
                    layer_configs.push(layer.clone());
                }
            }
        }
    }

    let mut futures = Vec::new();
    for layer in layer_configs {
        let pg_pool = match get_db_registry().get_pool(&layer.database_id) {
            Some(pool) => pool.clone(),
            None => continue,
        };
        futures.push(get_tile(
            pg_pool,
            layer,
            x,
            y,
            z,
            String::new(),
            Vec::new(),
        ));
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

    res.headers_mut().insert(
        "X-Cache",
        HeaderValue::from_str(&format!("HIT: {cache_hits}, MISS: {cache_misses}"))
            .unwrap_or_else(|_| HeaderValue::from_static("UNKNOWN")),
    );

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

    let catalog = get_catalog().await.read().await;

    let layers_vec = catalog.find_layers_by_category(&category, StateLayer::Published);

    let mut layer_configs = Vec::new();
    for layer in layers_vec {
        if validate_user_groups(req, layer, depot).await? {
            let zmin = layer.zmin.unwrap_or(0);
            let zmax = layer.zmax.unwrap_or(22);
            if z >= zmin && z <= zmax {
                layer_configs.push(layer.clone());
            }
        }
    }

    let mut futures = Vec::new();
    for layer in layer_configs {
        let pg_pool = match get_db_registry().get_pool(&layer.database_id) {
            Some(pool) => pool.clone(),
            None => continue,
        };
        futures.push(get_tile(
            pg_pool,
            layer,
            x,
            y,
            z,
            String::new(),
            Vec::new(),
        ));
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

    res.headers_mut().insert(
        "X-Cache",
        HeaderValue::from_str(&format!("HIT: {cache_hits}, MISS: {cache_misses}"))
            .unwrap_or_else(|_| HeaderValue::from_static("UNKNOWN")),
    );

    let final_output = Bytes::from(output_data.concat());
    res.body(salvo::http::ResBody::Once(final_output));

    Ok(())
}
