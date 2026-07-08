use std::collections::{BTreeMap, HashMap};

use salvo::http::{StatusCode, header::HeaderValue};
use salvo::prelude::*;
use serde::Serialize;
use tracing::warn;

use crate::{
    db::metadata::{query_extent, query_fields_with_comments},
    error::AppResult,
    get_catalog, get_public_url,
    models::catalog::{Layer, StateLayer},
    services::utils::validate_user_groups,
};

/// A single entry of the TileJSON 3.0.0 `vector_layers` array.
#[derive(Debug, Serialize)]
pub struct VectorLayer {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub minzoom: u32,
    pub maxzoom: u32,
    pub fields: BTreeMap<String, String>,
}

/// TileJSON 3.0.0 document.
/// https://github.com/mapbox/tilejson-spec/blob/master/3.0.0/README.md
#[derive(Debug, Serialize)]
pub struct TileJson {
    pub tilejson: String,
    pub tiles: Vec<String>,
    pub vector_layers: Vec<VectorLayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub scheme: String,
    pub minzoom: u32,
    pub maxzoom: u32,
    pub bounds: [f64; 4],
    pub center: [f64; 3],
}

/// Entry of the TileJSON index (`GET /services/tilejson`).
#[derive(Debug, Serialize)]
pub struct TileJsonIndexEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tilejson_url: String,
}

/// Base URL for absolute links: configured `server.public_url` wins,
/// otherwise derived from forwarded headers / the request itself.
pub fn resolve_base_url(
    public_url: Option<&str>,
    forwarded_proto: Option<&str>,
    forwarded_host: Option<&str>,
    scheme: &str,
    host: &str,
) -> String {
    if let Some(url) = public_url {
        return url.trim_end_matches('/').to_string();
    }
    let proto = forwarded_proto.unwrap_or(scheme);
    let host = forwarded_host.unwrap_or(host);
    format!("{proto}://{host}")
}

/// The layer's configured field list. Mirrors `utils::convert_fields`:
/// a single element may hold a comma-separated list.
pub fn configured_fields(layer: &Layer) -> Vec<String> {
    if layer.fields.len() == 1 {
        layer.fields[0]
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        layer.fields.iter().map(|s| s.trim().to_string()).collect()
    }
}

pub fn build_tilejson(
    layer: &Layer,
    bounds: [f64; 4],
    fields: BTreeMap<String, String>,
    base_url: &str,
) -> TileJson {
    let minzoom = layer.get_zmin();
    let maxzoom = layer.get_zmax();
    let name = if layer.alias.is_empty() {
        layer.name.clone()
    } else {
        layer.alias.clone()
    };
    let description = if layer.description.is_empty() {
        None
    } else {
        Some(layer.description.clone())
    };

    let center = [
        (bounds[0] + bounds[2]) / 2.0,
        (bounds[1] + bounds[3]) / 2.0,
        ((minzoom + maxzoom) / 2) as f64,
    ];

    TileJson {
        tilejson: "3.0.0".to_string(),
        tiles: vec![format!(
            "{base_url}/services/tiles/{}:{}/{{z}}/{{x}}/{{y}}.pbf",
            layer.category.name, layer.name
        )],
        vector_layers: vec![VectorLayer {
            id: layer.name.clone(),
            description: description.clone(),
            minzoom,
            maxzoom,
            fields,
        }],
        name: Some(name),
        description,
        scheme: "xyz".to_string(),
        minzoom,
        maxzoom,
        bounds,
        center,
    }
}

/// World bounds in EPSG:4326 (Web Mercator latitude limits), used when the
/// extent query fails so the document is still valid.
const WORLD_BOUNDS: [f64; 4] = [-180.0, -85.05112877980659, 180.0, 85.05112877980659];

fn base_url_from_request(req: &Request) -> String {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    resolve_base_url(
        get_public_url(),
        header("x-forwarded-proto").as_deref(),
        header("x-forwarded-host").as_deref(),
        req.uri().scheme_str().unwrap_or("http"),
        header("host").as_deref().unwrap_or("localhost"),
    )
}

/// Bounds for the layer; falls back to world bounds on error (never a 500).
async fn layer_bounds(layer: &Layer) -> [f64; 4] {
    match query_extent(layer).await {
        Ok(ext) => [ext.xmin, ext.ymin, ext.xmax, ext.ymax],
        Err(e) => {
            warn!(layer = %layer.name, error = ?e, "TileJSON: extent query failed, using world bounds");
            WORLD_BOUNDS
        }
    }
}

/// `{field: description}` map for the layer's configured fields.
/// Description is the PostgreSQL column comment, falling back to the type
/// name. On query failure returns an empty map (never a 500).
async fn layer_fields(layer: &Layer) -> BTreeMap<String, String> {
    let columns = match query_fields_with_comments(
        &layer.database_id,
        layer.schema.clone(),
        layer.table_name.clone(),
    )
    .await
    {
        Ok(columns) => columns,
        Err(e) => {
            warn!(layer = %layer.name, error = ?e, "TileJSON: field query failed, omitting fields");
            return BTreeMap::new();
        }
    };

    let by_name: HashMap<String, _> = columns
        .into_iter()
        .map(|c| (c.name.clone(), c))
        .collect();

    configured_fields(layer)
        .into_iter()
        .filter_map(|name| {
            by_name
                .get(&name)
                .map(|c| (name, c.description.clone().unwrap_or_else(|| c.udt.clone())))
        })
        .collect()
}

fn set_json_cache_headers(res: &mut Response) {
    res.headers_mut().insert(
        "Cache-Control",
        HeaderValue::from_static("public, max-age=3600"),
    );
}

#[handler]
pub async fn tilejson_layer(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let layer_name = req.param::<String>("layer_name").unwrap_or_default();
    let (category, name) = layer_name.split_once(':').unwrap_or(("", ""));

    let layer = {
        let catalog = get_catalog().await.read().await;
        catalog
            .find_layer_by_category_and_name(category, name, StateLayer::Published)
            .cloned()
    };

    let Some(layer) = layer else {
        warn!(category = %category, name = %name, "TileJSON: layer not found");
        res.status_code(StatusCode::NOT_FOUND);
        return Ok(());
    };

    if !validate_user_groups(req, &layer, depot).await? {
        warn!(category = %category, name = %name, "TileJSON: user not authorized for layer");
        res.status_code(StatusCode::FORBIDDEN);
        return Ok(());
    }

    let bounds = layer_bounds(&layer).await;
    let fields = layer_fields(&layer).await;
    let base_url = base_url_from_request(req);

    set_json_cache_headers(res);
    res.render(Json(build_tilejson(&layer, bounds, fields, &base_url)));
    Ok(())
}

#[handler]
pub async fn tilejson_index(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let layers = {
        let catalog = get_catalog().await.read().await;
        catalog.get_published_layers()
    };
    let base_url = base_url_from_request(req);

    let mut entries = Vec::new();
    for layer in layers {
        if !validate_user_groups(req, &layer, depot).await? {
            continue;
        }
        let id = format!("{}:{}", layer.category.name, layer.name);
        entries.push(TileJsonIndexEntry {
            name: if layer.alias.is_empty() {
                layer.name.clone()
            } else {
                layer.alias.clone()
            },
            description: layer.description.clone(),
            tilejson_url: format!("{base_url}/services/tilejson/{id}.json"),
            id,
        });
    }

    set_json_cache_headers(res);
    res.render(Json(entries));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::category::Category;

    fn test_layer() -> Layer {
        Layer {
            id: "layer-1".to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "public".to_string(),
                description: "".to_string(),
            },
            geometry: "polygons".to_string(),
            name: "parcels".to_string(),
            alias: "Parcels".to_string(),
            description: "Cadastral parcels".to_string(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "parcels".to_string(),
            fields: vec!["gid".to_string(), "owner".to_string()],
            filter: None,
            srid: None,
            geom: None,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: Some(4),
            zmax: Some(14),
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age: None,
            max_records: None,
            published: true,
            url: None,
            groups: None,
        }
    }

    #[test]
    fn resolve_base_url_prefers_public_url_and_strips_trailing_slash() {
        let url = resolve_base_url(
            Some("https://tiles.example.com/"),
            Some("http"),
            Some("internal:5887"),
            "http",
            "localhost:5887",
        );
        assert_eq!(url, "https://tiles.example.com");
    }

    #[test]
    fn resolve_base_url_uses_forwarded_headers() {
        let url = resolve_base_url(
            None,
            Some("https"),
            Some("tiles.example.com"),
            "http",
            "localhost:5887",
        );
        assert_eq!(url, "https://tiles.example.com");
    }

    #[test]
    fn resolve_base_url_falls_back_to_request_host() {
        let url = resolve_base_url(None, None, None, "http", "localhost:5887");
        assert_eq!(url, "http://localhost:5887");
    }

    #[test]
    fn configured_fields_splits_single_comma_separated_element() {
        let mut layer = test_layer();
        layer.fields = vec!["gid, owner ,area".to_string()];
        assert_eq!(configured_fields(&layer), vec!["gid", "owner", "area"]);
    }

    #[test]
    fn configured_fields_keeps_multiple_elements() {
        assert_eq!(configured_fields(&test_layer()), vec!["gid", "owner"]);
    }

    #[test]
    fn build_tilejson_maps_layer() {
        let mut fields = BTreeMap::new();
        fields.insert("gid".to_string(), "int4".to_string());
        fields.insert("owner".to_string(), "Owner full name".to_string());

        let doc = build_tilejson(
            &test_layer(),
            [-60.0, -40.0, -50.0, -30.0],
            fields,
            "http://localhost:5887",
        );

        assert_eq!(doc.tilejson, "3.0.0");
        assert_eq!(
            doc.tiles,
            vec!["http://localhost:5887/services/tiles/public:parcels/{z}/{x}/{y}.pbf"]
        );
        assert_eq!(doc.name.as_deref(), Some("Parcels"));
        assert_eq!(doc.description.as_deref(), Some("Cadastral parcels"));
        assert_eq!(doc.scheme, "xyz");
        assert_eq!(doc.minzoom, 4);
        assert_eq!(doc.maxzoom, 14);
        assert_eq!(doc.bounds, [-60.0, -40.0, -50.0, -30.0]);
        assert_eq!(doc.center, [-55.0, -35.0, 9.0]);

        assert_eq!(doc.vector_layers.len(), 1);
        let vl = &doc.vector_layers[0];
        assert_eq!(vl.id, "parcels");
        assert_eq!(vl.description.as_deref(), Some("Cadastral parcels"));
        assert_eq!(vl.minzoom, 4);
        assert_eq!(vl.maxzoom, 14);
        assert_eq!(vl.fields.get("owner").map(String::as_str), Some("Owner full name"));
    }

    #[test]
    fn build_tilejson_alias_fallback_and_empty_description_omitted() {
        let mut layer = test_layer();
        layer.alias = "".to_string();
        layer.description = "".to_string();

        let doc = build_tilejson(&layer, [0.0, 0.0, 1.0, 1.0], BTreeMap::new(), "http://h");

        assert_eq!(doc.name.as_deref(), Some("parcels"));
        assert!(doc.description.is_none());

        let json = serde_json::to_value(&doc).unwrap();
        assert!(json.get("description").is_none(), "None must be omitted, not null");
        assert!(json.get("tilejson").is_some());
        assert!(json.get("tiles").is_some());
        assert!(json.get("vector_layers").is_some());
    }
}
