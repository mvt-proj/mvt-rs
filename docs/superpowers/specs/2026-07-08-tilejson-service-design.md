# TileJSON 3.0.0 Service — Design

**Date:** 2026-07-08
**Status:** Approved
**Spec reference:** https://github.com/mapbox/tilejson-spec/blob/master/3.0.0/README.md

## Goal

Add a TileJSON 3.0.0 endpoint to MVT Server so clients (MapLibre, QGIS, OpenLayers)
can discover tile URLs, zoom range, bounds, and vector layer schema for each
published layer without manual configuration.

## Scope (v1)

- Per-layer TileJSON document: `GET /services/tilejson/{layer_name}.json`
  where `layer_name` is `category:name` (same format as the tile endpoint).
- Index: `GET /services/tilejson` returns a JSON array of
  `{id, name, description, tilejson_url}` for every published layer visible
  to the requesting user.

Out of scope for v1: TileJSON for the `multi/{layers}` and `category/{category}`
tile endpoints, `attribution`, `template`, `legend`, TileJSON caching.

## Architecture

New handler module `src/services/tilejson.rs` (same pattern as
`services/styles.rs` / `services/legends.rs`), registered in
`src/services/mod.rs` and wired in `build_services_routes()` in
`src/routes.rs`.

Documents are generated **on demand per request**: the layer comes from the
in-memory catalog (`get_catalog()`), plus two cheap PostGIS queries (estimated
extent and field list with comments). No new server state; always fresh after a
layer edit; consistent across cluster instances.

## Document mapping

| TileJSON field | Source |
|---|---|
| `tilejson` | literal `"3.0.0"` |
| `tiles` | `["{base_url}/services/tiles/{category:name}/{z}/{x}/{y}.pbf"]` |
| `scheme` | `"xyz"` |
| `name` | `layer.alias`, falling back to `layer.name` if empty |
| `description` | `layer.description` |
| `minzoom` / `maxzoom` | `layer.get_zmin()` / `layer.get_zmax()` |
| `bounds` | `query_extent(layer)` (existing, EPSG:4326) |
| `center` | bounds center `[lon, lat, zoom]` with `zoom = (zmin + zmax) / 2` |
| `vector_layers` | single element: `{id: layer.name, description, minzoom, maxzoom, fields}` |

`TileJson` is a `serde::Serialize` struct; optional fields use
`#[serde(skip_serializing_if = "Option::is_none")]` so absent values are
omitted rather than serialized as `null` (spec requirement).

`fields` only includes columns listed in the layer's configured `fields` list,
not every table column.

## Field descriptions

New function in `src/db/metadata.rs`:
`query_fields_with_comments(database_id, schema, table) -> Vec<FieldWithComment>`.
It extends the existing `pg_attribute` query with
`LEFT JOIN col_description(c.oid, a.attnum)`. Per field, description is the
PostgreSQL column comment when present, otherwise the `udt` type name
(e.g. `"varchar"`) as fallback — same convention as Martin / pg_tileserv.

## Base URL resolution

New optional setting `server.public_url` (env `MVT_SERVER__PUBLIC_URL`) in
`src/config/settings.rs`. Helper `resolve_base_url(req)`:

1. If `public_url` is configured → use it (trailing slash stripped).
2. Otherwise derive from the request:
   `{X-Forwarded-Proto | request scheme}://{X-Forwarded-Host | Host}`.

Document the setting in `config.example.yaml`, `.env.example`, and CLAUDE.md.

## Access control

Mirror the tile endpoint exactly: only `published` layers are served; when a
layer has groups, validate with `validate_user_groups` (JWT/session). The
index filters out layers the user cannot access. Unknown layer or missing
permission produces the same `AppError` responses as the tile endpoint
(404 / 401).

## Errors and headers

- `Content-Type: application/json`; `Cache-Control: public, max-age=3600`.
- `query_extent` failure → fall back to world bounds
  `[-180.0, -85.05112877980659, 180.0, 85.05112877980659]` and log a warning
  (same criterion as `page_map_layer`); never a 500.
- Field query failure → `fields: {}` with a warning.

## Testing

- Unit tests (own `tests` module in `src/services/tilejson.rs`):
  - `TileJson` serialization: mandatory fields (`tilejson`, `tiles`,
    `vector_layers`) present; `None` optionals omitted from JSON.
  - `resolve_base_url`: configured `public_url` wins; forwarded headers used
    otherwise; trailing slash handling.
  - Layer → document mapping (alias fallback, zoom range, center calculation).
- Manual verification of a served document against the 3.0.0 spec's required
  fields.
