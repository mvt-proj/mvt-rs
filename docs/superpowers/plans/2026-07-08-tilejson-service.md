# TileJSON 3.0.0 Service Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve TileJSON 3.0.0 documents for every published layer (`GET /services/tilejson/{category:name}.json`) plus a discovery index (`GET /services/tilejson`).

**Architecture:** New handler module `src/services/tilejson.rs` (same pattern as `services/styles.rs`). Documents are built on demand per request from the in-memory catalog plus two PostGIS queries: estimated extent (existing `query_extent`) and a new field query that joins PostgreSQL column comments. A new optional setting `server.public_url` controls the absolute base URL in the `tiles` array, falling back to request headers.

**Tech Stack:** Rust, Salvo 0.93, SQLx 0.8 (PostgreSQL), serde/serde_json.

**Spec:** `docs/superpowers/specs/2026-07-08-tilejson-service-design.md` and https://github.com/mapbox/tilejson-spec/blob/master/3.0.0/README.md

## Global Constraints

- TileJSON version string is exactly `"3.0.0"`.
- Mandatory document fields: `tilejson`, `tiles`, `vector_layers`. Optional fields that are `None` must be omitted from JSON (never `null`).
- Tile URL template: `{base_url}/services/tiles/{category}:{name}/{z}/{x}/{y}.pbf`.
- Only `published` layers are served; group-restricted layers require `validate_user_groups` (mirrors tile endpoint: 404 unknown, 403 unauthorized).
- Extent failure → world bounds `[-180.0, -85.05112877980659, 180.0, 85.05112877980659]` + warning, never a 500. Field-query failure → empty `fields` + warning.
- Field description = PostgreSQL column comment, fallback to the `udt` type name.
- All commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: `server.public_url` setting + global accessor

**Files:**
- Modify: `src/config/settings.rs` (ServerConfig, ~line 16-25; test fixture ~line 239)
- Modify: `src/main.rs` (statics block ~line 67-71; init ~line 199)
- Modify: `config.example.yaml` (server section, lines 11-13)

**Interfaces:**
- Produces: `settings.server.public_url: Option<String>`; `crate::get_public_url() -> Option<&'static str>` (returns `None` when unset or when the static was never initialized, e.g. in unit tests).

- [ ] **Step 1: Write the failing test**

In `src/config/settings.rs`, inside the existing `mod tests`, add:

```rust
    #[test]
    fn public_url_defaults_to_none() {
        let s = valid_settings();
        assert!(s.server.public_url.is_none());
        assert!(s.validate().is_ok());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::settings::tests::public_url_defaults_to_none`
Expected: COMPILE ERROR — `ServerConfig` has no field `public_url` (the fixture doesn't set it and the struct lacks it).

- [ ] **Step 3: Add the field**

In `src/config/settings.rs`, change `ServerConfig` to:

```rust
#[derive(Debug, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Public base URL used in absolute URLs (TileJSON `tiles` array),
    /// e.g. "https://tiles.example.com". When unset, the URL is derived
    /// from the request (X-Forwarded-Proto / X-Forwarded-Host / Host).
    pub public_url: Option<String>,
}
```

In the same file's `valid_settings()` test fixture, change the `server:` line to:

```rust
            server: ServerConfig { host: "0.0.0.0".to_string(), port: 5887, public_url: None },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib config::settings`
Expected: all settings tests PASS, including `public_url_defaults_to_none`.

- [ ] **Step 5: Add the global accessor in `src/main.rs`**

After the `CONFIG_DIR` static/getter block (ends ~line 71), add:

```rust
static PUBLIC_URL: OnceLock<Option<String>> = OnceLock::new();
#[inline]
pub fn get_public_url() -> Option<&'static str> {
    PUBLIC_URL.get().and_then(|url| url.as_deref())
}
```

Right after the line `CONFIG_DIR.set(settings.paths.config.clone()).unwrap();` (~line 199, before the cluster-mode branch so both init paths get it), add:

```rust
    PUBLIC_URL.set(settings.server.public_url.clone()).unwrap();
```

- [ ] **Step 6: Document in `config.example.yaml`**

Change the server section (lines 11-13) to:

```yaml
server:
  host: "0.0.0.0"
  port: 5887
  # Optional: public base URL used to build absolute URLs (e.g. the TileJSON
  # `tiles` array). Set this when running behind a proxy or load balancer.
  # When unset, URLs are derived from the request headers
  # (X-Forwarded-Proto / X-Forwarded-Host / Host).
  # Env var: MVT_SERVER__PUBLIC_URL
  # public_url: "https://tiles.example.com"
```

- [ ] **Step 7: Verify the whole crate still builds and tests pass**

Run: `cargo test`
Expected: PASS (no new failures).

- [ ] **Step 8: Commit**

```bash
git add src/config/settings.rs src/main.rs config.example.yaml
git commit -m "feat: add optional server.public_url setting

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `query_fields_with_comments` in db/metadata.rs

**Files:**
- Modify: `src/db/metadata.rs` (add struct after `Field` ~line 24, add function after `query_fields` ~line 119)

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub struct FieldWithComment { pub name: String, pub udt: String, pub description: Option<String> }` and `pub async fn query_fields_with_comments(database_id: &str, schema: String, table: String) -> AppResult<Vec<FieldWithComment>>`.

Note: this function needs a live PostGIS connection, so it has no unit test (same as every other function in this file — they're exercised via integration/manual testing). The compile check is the gate here.

- [ ] **Step 1: Add the struct**

In `src/db/metadata.rs`, after the `Field` struct (line 24), add:

```rust
#[derive(FromRow, Serialize, Debug)]
pub struct FieldWithComment {
    pub name: String,
    pub udt: String,
    pub description: Option<String>,
}
```

- [ ] **Step 2: Add the query function**

After `query_fields` (ends line 119), add:

```rust
pub async fn query_fields_with_comments(
    database_id: &str,
    schema: String,
    table: String,
) -> AppResult<Vec<FieldWithComment>> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

    let sql = r#"
        SELECT
            a.attname AS name,
            t.typname AS udt,
            col_description(c.oid, a.attnum) AS description
        FROM pg_attribute a
        JOIN pg_class c      ON a.attrelid = c.oid
        JOIN pg_namespace n  ON c.relnamespace = n.oid
        JOIN pg_type t       ON a.atttypid = t.oid
        WHERE n.nspname = $1
          AND c.relname = $2
          AND a.attnum > 0
          AND NOT a.attisdropped
        ORDER BY a.attnum;
    "#;

    let data = sqlx::query_as::<_, FieldWithComment>(sql)
        .bind(schema)
        .bind(table)
        .fetch_all(&pg_pool)
        .await?;

    Ok(data)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: success, no warnings about the new code.

- [ ] **Step 4: Commit**

```bash
git add src/db/metadata.rs
git commit -m "feat: add query_fields_with_comments (PostgreSQL column comments)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: TileJson model, base-URL resolution, and document builder (pure functions + tests)

**Files:**
- Create: `src/services/tilejson.rs` (model + pure functions + tests; handlers come in Task 4)
- Modify: `src/services/mod.rs` (register module)

**Interfaces:**
- Consumes: `crate::models::catalog::Layer` (fields: `name`, `alias`, `description`, `category.name`, `get_zmin()`, `get_zmax()`).
- Produces (used by Task 4):
  - `pub struct TileJson` / `pub struct VectorLayer` (Serialize)
  - `pub fn resolve_base_url(public_url: Option<&str>, forwarded_proto: Option<&str>, forwarded_host: Option<&str>, scheme: &str, host: &str) -> String`
  - `pub fn build_tilejson(layer: &Layer, bounds: [f64; 4], fields: BTreeMap<String, String>, base_url: &str) -> TileJson`
  - `pub fn configured_fields(layer: &Layer) -> Vec<String>`

- [ ] **Step 1: Register the module**

In `src/services/mod.rs`, add (keeping alphabetical order):

```rust
pub mod health;
pub mod legends;
pub mod styles;
#[cfg(test)]
mod tests;
pub mod tilejson;
pub mod tiles;
pub mod utils;
```

- [ ] **Step 2: Create `src/services/tilejson.rs` with structs and stub functions**

```rust
use std::collections::BTreeMap;

use serde::Serialize;

use crate::models::catalog::Layer;

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
    todo!()
}

/// The layer's configured field list. Mirrors `utils::convert_fields`:
/// a single element may hold a comma-separated list.
pub fn configured_fields(layer: &Layer) -> Vec<String> {
    todo!()
}

pub fn build_tilejson(
    layer: &Layer,
    bounds: [f64; 4],
    fields: BTreeMap<String, String>,
    base_url: &str,
) -> TileJson {
    todo!()
}
```

- [ ] **Step 3: Write the failing tests**

Append to `src/services/tilejson.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test --lib services::tilejson`
Expected: FAIL — every test panics with `not yet implemented` (`todo!()`).

- [ ] **Step 5: Implement the three functions**

Replace the three `todo!()` bodies:

```rust
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
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib services::tilejson`
Expected: 7 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/services/tilejson.rs src/services/mod.rs
git commit -m "feat: add TileJSON 3.0.0 document model and builder

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Handlers and routing

**Files:**
- Modify: `src/services/tilejson.rs` (add handlers on top of Task 3's functions)
- Modify: `src/routes.rs` (import ~line 18, `build_services_routes` ~line 275)

**Interfaces:**
- Consumes: `build_tilejson`, `resolve_base_url`, `configured_fields`, `TileJsonIndexEntry` (Task 3); `query_extent`, `query_fields_with_comments` (Task 2 / existing); `get_catalog()`, `get_public_url()`; `validate_user_groups(req, &layer, depot)` from `crate::services::utils`.
- Produces: `#[handler] pub async fn tilejson_layer` and `#[handler] pub async fn tilejson_index`, routed at `services/tilejson/{layer_name}.json` and `services/tilejson`.

No handler-level unit tests: handlers depend on global state (`get_catalog`, DB registry) that only exists at runtime — the same convention as `styles::index` and the tile handlers. The pure logic was tested in Task 3; handlers are verified by `cargo check` here and end-to-end in Task 5.

- [ ] **Step 1: Add handler imports to `src/services/tilejson.rs`**

Replace the existing `use` block at the top of the file with:

```rust
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
```

- [ ] **Step 2: Add the helpers and handlers**

Append (before the `#[cfg(test)]` module):

```rust
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
```

- [ ] **Step 3: Wire the routes**

In `src/routes.rs`, extend the services import (line 18):

```rust
    services::{health, legends, styles, tilejson, tiles::handlers as tiles},
```

In `build_services_routes` (~line 275), add the two routes after `legends`:

```rust
fn build_services_routes(settings: &Settings, cache: impl Handler) -> Router {
    Router::with_path("services")
        .hoop(cache)
        .push(build_tiles_routes())
        .push(Router::with_path("styles/{style_name}").get(styles::index))
        .push(Router::with_path("legends/{style_name}").get(legends::index))
        .push(Router::with_path("tilejson").get(tilejson::tilejson_index))
        .push(Router::with_path("tilejson/{layer_name}.json").get(tilejson::tilejson_layer))
        .push(
            Router::with_path("map_assets/{**path}").get(
                StaticDir::new([&settings.paths.assets])
                    .include_dot_files(false)
                    .defaults("index.html")
                    .auto_list(true),
            ),
        )
}
```

- [ ] **Step 4: Verify build and full test suite**

Run: `cargo test`
Expected: PASS. Also run `cargo clippy --all-targets` — no new warnings.

- [ ] **Step 5: Commit**

```bash
git add src/services/tilejson.rs src/routes.rs
git commit -m "feat: serve TileJSON 3.0.0 per layer plus discovery index

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: End-to-end verification and docs

**Files:**
- Modify: `CLAUDE.md` (project structure + optional settings sections)

**Interfaces:**
- Consumes: the running server (`cargo run` needs a PostGIS `default` database per `config/config.yaml`).

- [ ] **Step 1: Run the server and verify the endpoints manually**

```bash
cargo run
```

In another terminal (replace `public:parcels` with a real published `category:name` from the catalog; check `curl -s http://localhost:5887/services/tilejson` for available ids):

```bash
curl -s http://localhost:5887/services/tilejson | python3 -m json.tool
curl -s http://localhost:5887/services/tilejson/public:parcels.json | python3 -m json.tool
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:5887/services/tilejson/nope:nope.json
```

Expected:
- Index: JSON array; every entry has `id`, `name`, `description`, `tilejson_url`.
- Layer document: has `"tilejson": "3.0.0"`, `tiles` array with one absolute `.pbf` URL template, `vector_layers` with `fields` populated (column comments where set, type names otherwise), `bounds` matching the data, `minzoom`/`maxzoom` from the layer config. No `null` values anywhere.
- Unknown layer: `404`.
- Load the layer document URL in a MapLibre/QGIS client if available (optional sanity check: QGIS "Vector Tiles → New Generic Connection" accepts a TileJSON URL).

- [ ] **Step 2: Verify public_url override**

```bash
MVT_SERVER__PUBLIC_URL=https://tiles.example.com cargo run
curl -s http://localhost:5887/services/tilejson/public:parcels.json | grep tiles.example.com
```

Expected: the `tiles` URL and index `tilejson_url` start with `https://tiles.example.com/services/`.

- [ ] **Step 3: Document in CLAUDE.md**

In the "Project structure" tree, under `└── services/`, add after the `styles.rs` line:

```
    ├── tilejson.rs      # TileJSON 3.0.0 documents (per layer + index)
```

In the "**Optional security settings:**" area of the Configuration section, add a sibling bullet list item under a new heading line:

```markdown
**Optional server settings:**
- `server.public_url` — public base URL for absolute URLs in TileJSON responses (default: derived from request headers). Env var: `MVT_SERVER__PUBLIC_URL`
```

- [ ] **Step 4: Full suite once more**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document TileJSON service and server.public_url

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```
