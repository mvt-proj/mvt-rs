// Published-layer guard and autofill derivation (Phase 2, tasks 2.5-2.8).
// The guard is a single pure function shared by both HTML and API write
// paths (design decision #9); autofill mirrors the world-bounds fallback
// used by `services::tilejson` so a dead PostGIS never 500s (decision #3).
#![allow(dead_code)]

use crate::error::{AppError, AppResult};
use crate::models::catalog::Layer;
use crate::models::metadata::MetadataLink;
use crate::services::metadata::ogc::PROTOCOL_WWW_LINK;

/// Rejects create/edit of a layer's metadata record when the layer itself
/// is not published (proposal hard rule; design decision #9). Callable
/// from both `html::admin::metadata` and `api::metadata` write paths.
pub fn guard_layer_published(layer: &Layer) -> AppResult<()> {
    if !layer.published {
        return Err(AppError::InvalidInput(format!(
            "layer '{}' must be published before it can have a metadata record",
            layer.name
        )));
    }
    Ok(())
}

/// World bounds in EPSG:4326, mirrored verbatim from
/// `services::tilejson::WORLD_BOUNDS` (design decision #3) so a dead
/// PostGIS never breaks metadata discovery either.
pub const WORLD_BOUNDS: [f64; 4] = [-180.0, -85.05112877980659, 180.0, 85.05112877980659];

/// Fields derived at read time from the live `Layer`, never stored on
/// `MetadataRecord` (design decision #3 — single source of truth).
#[derive(Debug, Clone, PartialEq)]
pub struct Autofill {
    pub title: String,
    pub abstract_text: String,
    /// `EPSG:{srid}`, derived from `layer.get_srid()` (defaults to 4326).
    /// Never stored on `MetadataRecord` — read-only display data, same
    /// single-source-of-truth treatment as `title`/`abstract_text` (design
    /// decision #3).
    pub projection: String,
    pub own_links: Vec<MetadataLink>,
}

/// Derives title/abstract/own-links from `layer`. `base_url` is the
/// already-resolved absolute base (see `services::tilejson::resolve_base_url`)
/// used to build the own tile/TileJSON links. Pure — does not query bbox;
/// see [`bbox_for_layer`] for that (I/O).
pub fn derive_autofill(layer: &Layer, base_url: &str) -> Autofill {
    let title = if layer.alias.is_empty() {
        layer.name.clone()
    } else {
        layer.alias.clone()
    };

    let id = format!("{}:{}", layer.category.name, layer.name);

    let own_links = vec![
        MetadataLink {
            id: format!("own-tiles-{id}"),
            protocol: PROTOCOL_WWW_LINK.to_string(),
            url: format!("{base_url}/services/tiles/{id}/{{z}}/{{x}}/{{y}}.pbf"),
            label: Some("Tiles (XYZ)".to_string()),
        },
        MetadataLink {
            id: format!("own-tilejson-{id}"),
            protocol: PROTOCOL_WWW_LINK.to_string(),
            url: format!("{base_url}/services/tilejson/{id}.json"),
            label: Some("TileJSON".to_string()),
        },
    ];

    Autofill {
        title,
        abstract_text: layer.description.clone(),
        projection: format!("EPSG:{}", layer.get_srid()),
        own_links,
    }
}

/// Bbox for `layer`; falls back to [`WORLD_BOUNDS`] on error (never a 500),
/// mirroring `services::tilejson::layer_bounds` verbatim (design decision
/// #3 — reuse, don't reinvent). Same DB dependency as its tilejson
/// counterpart, so — like `layer_bounds` — it has no unit-test coverage in
/// this codebase (the global `DbRegistry` is only initialized by `main()`);
/// it is exercised end-to-end by Phase 3/5 integration tests once wired.
pub async fn bbox_for_layer(layer: &Layer) -> [f64; 4] {
    match crate::db::metadata::query_extent(layer).await {
        Ok(ext) => [ext.xmin, ext.ymin, ext.xmax, ext.ymax],
        Err(e) => {
            tracing::warn!(
                layer = %layer.name,
                error = ?e,
                "metadata: extent query failed, using world bounds"
            );
            WORLD_BOUNDS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::category::Category;
    use crate::models::metadata::MetadataLink;

    #[test]
    fn derive_autofill_maps_title_from_alias_and_abstract_from_description() {
        let layer = test_layer(true);
        let autofill = derive_autofill(&layer, "http://localhost:5887");
        assert_eq!(autofill.title, "Parcels");
        assert_eq!(autofill.abstract_text, "Cadastral parcels");
    }

    #[test]
    fn derive_autofill_falls_back_to_layer_name_when_alias_is_empty() {
        let mut layer = test_layer(true);
        layer.alias = String::new();
        let autofill = derive_autofill(&layer, "http://localhost:5887");
        assert_eq!(autofill.title, "parcels");
    }

    #[test]
    fn derive_autofill_derives_projection_from_layer_srid() {
        let mut layer = test_layer(true);
        layer.srid = Some(3857);
        let autofill = derive_autofill(&layer, "http://localhost:5887");
        assert_eq!(autofill.projection, "EPSG:3857");
    }

    #[test]
    fn derive_autofill_projection_defaults_to_epsg_4326_when_srid_is_unset() {
        let layer = test_layer(true);
        assert_eq!(layer.srid, None);
        let autofill = derive_autofill(&layer, "http://localhost:5887");
        assert_eq!(autofill.projection, "EPSG:4326");
    }

    #[test]
    fn derive_autofill_derives_own_tile_and_tilejson_links() {
        let layer = test_layer(true);
        let autofill = derive_autofill(&layer, "http://localhost:5887");

        assert_eq!(autofill.own_links.len(), 2);
        let tile_link: &MetadataLink = autofill
            .own_links
            .iter()
            .find(|l| l.url.ends_with(".pbf"))
            .expect("must derive a tile link");
        assert_eq!(tile_link.protocol, "WWW:LINK-1.0-http--link");
        assert_eq!(
            tile_link.url,
            "http://localhost:5887/services/tiles/public:parcels/{z}/{x}/{y}.pbf"
        );

        let tilejson_link: &MetadataLink = autofill
            .own_links
            .iter()
            .find(|l| l.url.ends_with(".json"))
            .expect("must derive a tilejson link");
        assert_eq!(
            tilejson_link.url,
            "http://localhost:5887/services/tilejson/public:parcels.json"
        );
    }

    fn test_layer(published: bool) -> Layer {
        Layer {
            id: "layer-1".to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "public".to_string(),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "parcels".to_string(),
            alias: "Parcels".to_string(),
            description: "Cadastral parcels".to_string(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "parcels".to_string(),
            fields: vec!["gid".to_string()],
            filter: None,
            srid: None,
            geom: None,
            label_layer: false,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age: None,
            max_records: None,
            published,
            url: None,
            groups: None,
        }
    }

    #[test]
    fn guard_layer_published_rejects_unpublished_layer() {
        let err = guard_layer_published(&test_layer(false))
            .expect_err("must reject metadata create/edit on an unpublished layer");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn guard_layer_published_allows_published_layer() {
        assert!(guard_layer_published(&test_layer(true)).is_ok());
    }
}
