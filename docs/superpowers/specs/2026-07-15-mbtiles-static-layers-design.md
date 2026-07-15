# MBTiles static layers — design

**Status:** approved design, implementation paused (resume when scheduled).
**Scope:** MBTiles only. PMTiles deliberately deferred — see "Future: PMTiles" below.

## Motivation

mvt-rs currently serves tiles exclusively by generating MVT on the fly from PostGIS.
There's a recurring need to publish pre-rendered/offline tilesets (e.g. exports from
tippecanoe/ogr2ogr, third-party datasets) without standing up a PostGIS table for them.
MBTiles (a SQLite container of pre-rendered tiles) is the natural first format to
support: the project already depends on `sqlx` with the `sqlite` feature (used for the
config DB), so no new dependency is needed.

**Guiding constraint from the user:** stay as close as possible to the existing way of
working. Concretely:
- No new upload subsystem. Static tile files are deployed to the server filesystem by
  a sysadmin, the same way sprites/glyphs already work via `paths.assets`
  (see `src/main.rs` `MAP_ASSETS_DIR` / `get_map_assets()`).
- No hot-reload. A new/replaced `.mbtiles` file is picked up on server restart, same as
  config changes today have no live-reload story either.
- One catalog, one Layers UI. MBTiles layers live in the same `layers` table and the
  same admin screens as PostGIS layers, distinguished by a `source_kind` field — not a
  separate "Static Tilesets" section.

## Data model

New config setting: `paths.static_tiles` (default e.g. `static_tiles/`), same shape as
`paths.assets` in `src/config/settings.rs`.

Migration adds to the `layers` table:
- `source_kind TEXT NOT NULL DEFAULT 'postgis'` — `'postgis'` | `'mbtiles'`
- `static_file TEXT` — filename relative to `paths.static_tiles` (e.g. `roads_2026.mbtiles`);
  only meaningful when `source_kind = 'mbtiles'`

Existing PostGIS-only columns (`schema`, `table_name`, `fields`, `filter`, `sql_mode`,
`buffer`, `extent`, `clip_geom`, `database_id`) become optional at the Rust level and are
`NULL`/ignored for `mbtiles` layers. No column is removed or repurposed.

`src/models/catalog.rs`:
```rust
pub enum SourceKind { Postgis, Mbtiles }
// forward-compatible: a future Pmtiles variant slots in here without
// another migration — static_file/get_static_path stay format-agnostic.

pub source_kind: SourceKind,
pub static_file: Option<String>,
```
Helper `get_static_path() -> PathBuf` joins `paths.static_tiles` + `static_file`, mirroring
`get_map_assets()`.

`src/config/layers.rs`: `create_layer`/`update_layer`/`get_layers` read/write the two new
columns; no structural change to the rest of the query.

## Serving pipeline

New `StaticTileRegistry` (parallel to `DbRegistry` in `src/db/connection.rs`): at startup,
scans the catalog for layers with `source_kind = Mbtiles`, opens one read-only `SqlitePool`
per distinct `static_file` (a file can be referenced by more than one layer), stored behind
a `OnceLock` like other global state. Because refresh = restart, no reopen/invalidation
logic is needed.

`get_single_layer_tile` (`src/services/tiles/handlers.rs`) branches on `layer.source_kind`
right after the catalog lookup:
- **Postgis**: unchanged — resolve pool via `get_db_registry()`, existing `get_tile()` path.
- **Mbtiles**: resolve pool via `StaticTileRegistry` by `layer.static_file` instead of
  `DbRegistry`. Zoom check (`zmin`/`zmax`), group auth, and ETag computation (via
  `get_cache_wrapper().get_layer_version()`) are **reused unchanged** — that version
  counter tracks config edits, not tile bytes, so it applies equally here.
  `cache_wrapper.get_tile`/`set_tile` (the Redis/disk tile-byte cache) is **skipped** for
  mbtiles layers: reading a local SQLite file is already fast, and adding a second cache
  layer on top has no real payoff for this source type.
  - Scheme conversion: routes use XYZ (top-left origin, same as PostGIS/`ST_TileEnvelope`
    today); MBTiles stores TMS (bottom-left origin), so convert
    `tms_y = (1 << z) - 1 - y` before `SELECT tile_data FROM tiles WHERE zoom_level=? AND
    tile_column=? AND tile_row=?`.
  - No row → `204 No Content` (empty tile), consistent with how an empty MVT is already
    handled elsewhere.
  - Many `.mbtiles` files store `tile_data` gzip-compressed. Detect via magic bytes
    (`0x1f 0x8b`) and set `Content-Encoding: gzip` instead of decompressing server-side —
    transparent to browsers/CDNs, avoids extra CPU per request.

## TileJSON / metadata

`build_tilejson()` (`src/services/tilejson.rs:84`) is already source-agnostic: it takes a
resolved `bounds: [f64; 4]` and `fields: BTreeMap<String, String>` and assembles the
document identically for any layer. `minzoom`/`maxzoom` already come from `layer.zmin`/
`zmax` (catalog config), not from the underlying source — no change needed there.

Only two upstream functions are source-dependent, and both already fail gracefully
(never a 500):
- `layer_bounds(layer)`: for `Mbtiles`, read the `bounds` row from the file's `metadata`
  table (via the same pool `StaticTileRegistry` already has open) and parse the CSV
  `"west,south,east,north"` → `[f64; 4]`. Missing/malformed falls back to the existing
  `WORLD_BOUNDS` constant.
- `layer_fields(layer)`: for `Mbtiles`, read the `json` row from `metadata` (the standard
  TileJSON `vector_layers[].fields` shape that tippecanoe/ogr2ogr already write on
  export), take the **first** `vector_layers` entry's field map. Missing/unparseable
  falls back to an empty map, same as the existing PostGIS failure path.

`vector_layers[0].id` in the final document stays `layer.name` from the catalog, not
whatever internal id the file's `json.vector_layers[].id` has — the public layer name is
catalog-owned regardless of source, consistent with how PostGIS layers work today.

**Known limitation (not solved now):** an `.mbtiles` whose `json.vector_layers` has more
than one entry only exposes the first one's fields in TileJSON. mvt-rs models one catalog
layer as one named layer, matching the existing PostGIS assumption (one table per layer).

## Admin UI

`templates/admin/catalog/layers/{new,edit}.html`: add a `source_kind` select (PostGIS |
MBTiles) below "geometry". Plain JS (no network roundtrip) shows/hides two existing-style
blocks based on the selection:
- **PostGIS** block: unchanged — `database_id` → `schema` → `table` → `fields` →
  `filter`/`sql_mode`/`buffer`/`extent`/`clip_geom`.
- **MBTiles** block (new): single `static_file` select, populated via `hx-get` the same
  way `schema`/`table` are today, but backed by a filesystem listing instead of a
  PostGIS query.

New endpoint `GET /admin/database/static_files`, added next to `schemas`/`tables`/`fields`
in `build_admin_database_routes()` (`src/routes.rs`): lists `*.mbtiles` files under
`paths.static_tiles` via `std::fs::read_dir`, returns an `<option>` HTML fragment — same
shape as `html::admin::database::schemas`, reading the filesystem instead of PostGIS.

`create_layer`/`update_layer` (`src/html/admin/catalog.rs`): when `source_kind = mbtiles`,
clear the PostGIS-only fields before insert; when `source_kind = postgis`, `static_file`
stays `NULL`. Plain if/else validation, no new framework.

Layers list template: add a small badge ("PostGIS" / "MBTiles") per row so the two kinds
are visually distinguishable.

Fields shared unchanged across both kinds: category, groups, published, zmin/zmax,
max_cache_age, delete_cache_on_start.

## Testing

Unit tests (style matching `services/tilejson.rs` / `services/tiles/tests.rs`):
- `SourceKind` (de)serialization, default `postgis`.
- XYZ→TMS conversion, including the z=0 edge case.
- `metadata` table `bounds`/`json` parsing: valid, missing, malformed (must hit existing
  fallbacks, never panic/500).
- Gzip magic-byte detection.

Integration test: a small fixture `.mbtiles` under `tests/fixtures/`, exercising: create
layer via admin form (`source_kind=mbtiles`) → tile z/x/y returns OK → nonexistent tile →
204 → out-of-range zoom → 400 → unauthorized group → 403 → `tilejson` bounds/fields
correctness.

## Explicitly out of scope for this iteration

- PMTiles support.
- Hot-reload / refresh without server restart.
- File upload via the admin UI.
- Remote/URL-based static sources (S3, HTTP range requests).
- Multi-vector-layer `.mbtiles` field aggregation (only the first `vector_layers` entry's
  fields are used).

## Future: PMTiles

Deferred, not designed in detail here. The `SourceKind` enum and `static_file`/
`get_static_path()` are kept format-agnostic on purpose so that adding PMTiles later is a
new enum variant + a new branch in the tile-serving handler + a new metadata reader,
reusing the same `layers` table columns, the same admin UI selector, and the same catalog
model — no second migration, no parallel UI section.
