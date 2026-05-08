# MVT-RS — Claude Code Guide

## What this project is

MVT-RS is a high-performance vector tile server written in Rust. It serves Mapbox Vector Tiles (`.pbf`) from PostGIS databases, supports multi-database setups, Redis/disk caching, JWT + session authentication, and includes a web-based admin panel.

## Tech stack

| Layer | Technology |
|---|---|
| Web framework | [Salvo](https://salvo.rs/) 0.93 |
| Async runtime | Tokio 1 |
| DB driver | SQLx 0.8 (PostgreSQL + SQLite) |
| Cache | Redis (bb8) or disk |
| Templates | Askama (compile-time HTML) |
| Auth | JWT (jsonwebtoken) + Argon2 passwords + sessions |
| i18n | Fluent |
| Metrics | Prometheus |

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
│   └── database.rs      # GET /api/admin/database/...
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
├── config/
│   ├── settings.rs      # Settings struct (YAML + env vars via config crate)
│   ├── db.rs            # SQLite init for config storage
│   └── {users,groups,categories,layers,styles}.rs  # CRUD for SQLite config tables
│
├── db/
│   ├── connection.rs    # DbRegistry — manages named PgPool instances
│   └── metadata.rs      # Schema/table/field/SRID/extent queries
│
├── filters/             # Query parameter parsing → safe SQL WHERE clauses
├── html/                # HTML page handlers (Askama templates)
│   ├── admin/           # Admin panel pages
│   └── ...
├── models/              # Catalog, Category, Style data models
├── monitor/             # Prometheus metrics + SSE dashboard
└── services/
    ├── tiles/           # Tile generation pipeline (single, multi-layer, category)
    ├── styles.rs        # Style JSON endpoint
    ├── legends.rs       # Legend endpoint
    └── utils.rs         # SQL injection validation, field conversion
```

## Global state pattern

Application state is stored in `OnceLock` / `OnceCell` statics in `main.rs`, initialized once at startup. Access via inline getter functions:

```rust
get_db_registry()    // &'static DbRegistry
get_cf_pool()        // &'static SqlitePool  (config DB)
get_auth()           // &'static RwLock<Auth>
get_catalog()        // &'static RwLock<Catalog>
get_categories()     // &'static RwLock<Vec<Category>>
get_cache_wrapper()  // &'static CacheWrapper
get_jwt_secret()     // &'static String
get_map_assets()     // &'static String  (path)
```

## Error handling

`AppError` in `src/error.rs` implements `salvo::Writer` — it inspects the `Accept` header and renders either an HTML error page or a JSON response. All handlers return `AppResult<T>` (`Result<T, AppError>`).

## Configuration

Config is loaded in priority order: CLI args > YAML file > Environment variables > defaults.

- YAML: `config/config.yaml` (path overridable with `--config`)
- Env vars: `MVT_SERVER__PORT=5887`, `MVT_POSTGRES_DATABASES__DEFAULT=postgres://...`
- See `.env.example` for all available variables

**Required at startup:**
- `postgres_databases.default` must exist
- `security.session_secret` ≥ 32 chars
- `security.jwt_secret` ≥ 32 chars

## Running locally

```bash
# With cargo
cargo run

# With a specific config file
cargo run -- --config /path/to/config.yaml

# With docker-compose (includes PostGIS + Redis)
docker-compose up -d
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
