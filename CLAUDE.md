# MVT-RS — Claude Code Guide

## What this project is

MVT-RS is a high-performance vector tile server written in Rust. It serves Mapbox Vector Tiles (`.pbf`) from PostGIS databases, supports multi-database setups, Redis/disk caching, JWT + session authentication, and includes a web-based admin panel.

## Tech stack

| Layer | Technology |
|---|---|
| Web framework | [Salvo](https://salvo.rs/) 1.0 |
| Async runtime | Tokio 1 |
| DB driver | SQLx 0.9 (PostgreSQL + SQLite) |
| Cache | Redis (bb8) or disk |
| Templates | Askama (compile-time HTML) |
| Auth | JWT (jsonwebtoken) + Argon2 passwords + sessions |
| i18n | Fluent |
| Metrics | Prometheus |
| Plugin scripting | Lua 5.5 (mlua, vendored) — sandboxed per-layer/category SQL `WHERE` filters, see `docs/plugins.md` |

Check `Cargo.toml` for exact pinned versions; the table above gives the rough shape, not the source of truth.

## Project structure

```
src/
├── main.rs              # Entry point, global state init (OnceLock/OnceCell)
├── routes.rs            # All routes, middleware builders, CORS, session, cache
├── error.rs             # AppError enum + Writer impl (HTML or JSON responses)
├── i18n.rs              # Fluent i18n service
│
├── api/                 # JSON REST API handlers
│   ├── catalog.rs       # GET/POST /api/catalog/layer
│   ├── users.rs         # POST /api/users/login, GET/POST /api/admin/users
│   ├── database.rs      # GET /api/admin/database/...
│   ├── categories.rs, groups.rs, metadata.rs, styles.rs  # Admin CRUD endpoints
│   └── plugins.rs       # Lua plugin admin endpoints
│
├── auth/
│   ├── models.rs        # Auth, User, Group structs + login/password logic
│   ├── handlers.rs      # session_auth_handler, validate_token, jwt_auth_handler
│   └── utils.rs         # Basic auth decoding
│
├── cache/
│   ├── cachewrapper.rs  # Unified CacheWrapper (delegates to Redis or disk)
│   ├── redis.rs         # Redis cache backend
│   └── disk.rs          # Disk cache backend
│
├── cluster/             # Multi-instance config sync (standalone|shared|owner|client), see docs/clustering.md
│   ├── bootstrap.rs     # Startup: pulls config from the owner (client mode)
│   ├── watcher.rs       # Background poll for config changes (start_config_watcher, wired in main.rs)
│   ├── backend.rs, api.rs, snapshot.rs  # Internal /internal/config/* sync API
│
├── config/
│   ├── settings.rs      # Settings struct (YAML + env vars via config crate)
│   ├── db.rs            # SQLite init for config storage
│   ├── system_settings.rs  # Singleton settings row (e.g. plugins_version for hot-reload)
│   └── {users,groups,categories,layers,styles,metadata}.rs  # CRUD for SQLite config tables
│
├── db/
│   ├── connection.rs    # DbRegistry — manages named PgPool instances
│   └── metadata.rs      # Schema/table/field/SRID/extent queries, spatial index check
│
├── filters/             # Query parameter parsing → safe SQL WHERE clauses
├── html/                # HTML page handlers (Askama templates)
│   ├── admin/           # Admin panel pages
│   └── ...
├── models/              # Catalog, Category, Style data models
├── monitor/             # Prometheus metrics + SSE dashboard
├── plugins/             # Lua plugin registry (mlua) — LuaPluginRegistry::call_filter() per layer/category
│   ├── mod.rs
│   └── watcher.rs       # Hot-reload poll, NOT currently wired into the module tree (dead code, no `mod watcher;` in mod.rs)
└── services/
    ├── tiles/           # Tile generation pipeline (single, multi-layer, category)
    ├── health.rs        # /health endpoint
    ├── mvt_url.rs       # Rewrites the `mvt://` placeholder scheme to absolute URLs when serving a style
    ├── styles.rs        # Style JSON endpoint
    ├── tilejson.rs      # TileJSON 3.0.0 documents (per layer + index)
    ├── legends.rs       # Legend endpoint
    └── utils.rs         # SQL injection validation, field conversion
```

## Global state pattern

Application state is stored in statics in `main.rs`, initialized once at startup. Two flavors:

- **`std::sync::OnceLock`** for values set once at startup and never reloaded — sync getters, no `.await`:
```rust
get_db_registry()              // &'static DbRegistry
get_cf_pool()                   // &'static SqlitePool  (config DB)
get_cache_wrapper()             // &'static CacheWrapper
get_plugin_registry()           // &'static plugins::LuaPluginRegistry
get_jwt_secret()                // &'static String
get_cluster_secret()            // &'static str
get_map_assets()                // &'static String  (path)
get_sqlite_path(), get_config_dir(), get_public_url(), get_cache_invalidation_delay()
```
- **`tokio::sync::OnceCell<RwLock<T>>`** for in-memory state that gets reloaded at runtime (admin edits a layer/user/style) — async getters, need `.await`, then `.read()`/`.write()` on the inner `RwLock`:
```rust
get_catalog().await      // &'static RwLock<Catalog>
get_categories().await   // &'static RwLock<Vec<Category>>
get_auth().await         // &'static RwLock<Auth>
get_styles_cache().await // &'static RwLock<Vec<Style>>
```

## Error handling

`AppError` in `src/error.rs` implements `salvo::Writer` — it inspects the `Accept` header and renders either an HTML error page or a JSON response. All handlers return `AppResult<T>` (`Result<T, AppError>`).

## Configuration

Config is loaded in priority order: CLI args > YAML file > Environment variables > defaults.

- YAML: `config/config.yaml` (path overridable with `--config`)
- Env vars: `MVT_SERVER__PORT=5887`, `MVT_POSTGRES_DATABASES__DEFAULT=postgres://...`
- See `config.example.yaml` for all available settings and their env var equivalents

**Required at startup:**
- `postgres_databases.default` must exist
- `security.session_secret` ≥ 32 chars
- `security.jwt_secret` ≥ 32 chars

**Optional security settings:**
- `security.session_duration_minutes` — session TTL in minutes (default: `20`). Env var: `MVT_SECURITY__SESSION_DURATION_MINUTES`

**Optional server settings:**
- `server.public_url` — public base URL for absolute URLs in TileJSON responses (default: derived from request headers). Env var: `MVT_SERVER__PUBLIC_URL`

**Clustering (optional):** `cluster.mode` (`standalone` | `shared` | `owner` | `client`) keeps the in-memory config (catalog, categories, users, groups, styles) fresh across several instances behind a load balancer. Non-standalone modes require a shared Redis cache. See `config.example.yaml`'s `cluster:` section and `docs/clustering.md`.

## Running locally

```bash
# With cargo
cargo run

# With a specific config file
cargo run -- --config /path/to/config.yaml

# With docker-compose (includes PostGIS + Redis) — see docker-example/DOCKER_README.md
cd docker-example && docker-compose up -d
```

## Common workflows

### Add a new API endpoint
1. Add handler in `src/api/`
2. Wire it in `src/routes.rs` inside the appropriate builder function
3. If auth required, add `.hoop(auth::validate_token)` or `.hoop(auth::jwt_auth_handler())`

### Add a new admin page
1. Create Askama template in `templates/admin/`
2. Add handler in `src/html/admin/`
3. Wire in `build_admin_routes()` in `src/routes.rs`

### Add a database field to config
SQLite config tables are managed via `src/config/*.rs` with raw SQLx queries and migrations in `migrations/`.

## Testing

```bash
cargo test                    # all unit tests
cargo test -- --nocapture     # with stdout
```

Tests live in `src/*/tests.rs` modules. Integration tests are in `tests/integration/`.

## Documentation & i18n

User-facing docs (`README.md`, `TUTORIAL.md`) support translations via
suffix files: `README.<lang>.md`, `TUTORIAL.<lang>.md` (e.g. `README.es.md`).

- English files are the source of truth; keep translations in sync manually.
- Each file starts with a language-switcher line linking to its siblings.
- Shared assets (`docs/*.png`, `docs/*.svg`) are not duplicated per language.
- `docs/clustering.md` and `docs/plugins.md` remain English-only for now.
