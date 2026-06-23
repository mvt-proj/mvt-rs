# Multi-Instance Clustering Design

**Date:** 2026-06-23
**Status:** Approved (design), pending implementation plan
**Branch:** `feat/multi-instance-clustering`

## Goal

Keep the in-memory config state (`Catalog`, `Categories`, `Auth`, `Styles`)
fresh across multiple `mvt-rs` instances running behind a load balancer, covering
two deployment situations:

1. **Same host** (jails / docker on one machine) — instances share one SQLite
   config file via a shared volume.
2. **Different hosts** — instances run on separate machines that cannot share a
   single SQLite file safely.

A config edit on one instance must propagate to all the others **without
restarting them**.

## Hard constraint

There is **always a single SQLite database** for the configuration. No Postgres
or Redis as a config store, and no per-host SQLite copies. The data PostGIS is
remote / third-party-managed and is out of scope for config storage.

## Architecture overview

The config source of truth is one SQLite file. The in-memory state derives from
it. The mechanism to keep instances fresh is a **monotonic `config_version`
counter** stored in the existing `system_settings` table, bumped on every config
write, and a **background watcher** in each instance that reloads in-memory state
when the version increases.

What differs between the two situations is **how an instance reads the version
and the config data**:

- **Same host:** every instance opens the *same* SQLite file (shared volume), so
  it reads version and data locally.
- **Different hosts:** only one instance (the **owner**) holds the single SQLite
  file. The others (**clients**) have *no SQLite at all*; they obtain version and
  a full config snapshot from the owner over an authenticated internal HTTP API.

This mirrors the GeoServer master/slave model: a single writable owner plus
read-only clients.

## Deployment modes

Selected via `cluster.mode`:

| mode | local SQLite | watcher | internal API | serves | role |
|---|---|---|---|---|---|
| `standalone` (default) | initializes | — | — | everything (current behavior) | single instance |
| `shared` (situation 1) | initializes (shared volume) | yes — `LocalBackend` | — | everything | symmetric peers |
| `owner` (situation 2) | initializes | **no** | **yes** | everything | sole writer |
| `client` (situation 2) | **does not initialize** | yes — `RemoteBackend` | — | tiles + styles + legends | read-only |

All four in-memory states (`Catalog`, `Categories`, `Auth`, `Styles`) are treated
uniformly: every admin-managed item type (users, groups, categories, catalog
layers, and styles) is cached in memory, bumps `config_version` on write, travels
in the snapshot, and is rebuilt by the watcher. There is no special-casing.

Notes:

- **`owner` runs no watcher**: it is the only writer and config handlers already
  update in-memory state inline on write, so its state is always fresh. It only
  exposes the internal API.
- **`shared` runs the watcher on every instance**: any peer can write, so each
  must poll the shared counter.
- **`client` does not initialize SQLite** (no migrations, no admin-user
  creation). It bootstraps in-memory state from the owner snapshot at startup.

## Load balancer (nginx) routing — situation 2

Writes only ever reach the owner; clients only serve reads. This is enforced at
the load balancer, not in the app (no proxy code needed):

```nginx
upstream mvt_owner { server owner-host:5887; }            # owner (has the SQLite)
upstream mvt_tiles { server owner-host:5887;              # read pool
                     server client1-host:5887;
                     server client2-host:5887; }

# admin panel + login + config API  =>  ALWAYS the owner
location /admin     { proxy_pass http://mvt_owner; }
location /auth      { proxy_pass http://mvt_owner; }
location /api/admin { proxy_pass http://mvt_owner; }

# tiles / styles / legends (reads, all served from memory)  =>  balanced pool
location /          { proxy_pass http://mvt_tiles; }

# /internal must NOT be routed publicly
```

Login hits the owner; the session cookie works on any instance because sessions
are stateless (`CookieStore` signed with the shared `session_secret`), so no
sticky sessions are required.

## Components

### `config_version` counter

In `system_settings` (SQLite). Two helpers (mirroring the existing
`plugins_version` ones, but returning `Result<_, sqlx::Error>` to compose with
the `config/*.rs` write functions):

- `get_config_version(pool) -> Result<i64, sqlx::Error>`
- `bump_config_version(pool) -> Result<i64, sqlx::Error>`

Every cached-config write bumps it: categories, layers, users, groups, **and
styles** (`create_style`/`update_style`/`delete_style`). Styles are now cached in
memory like the others (see below), so they participate in the sync.

### `ConfigSnapshot`

The unit that travels over the API and that gets swapped into memory. All four
fields already derive `Serialize`/`Deserialize`:

```rust
#[derive(Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub catalog: Catalog,
    pub categories: Vec<Category>,
    pub auth: Auth,            // includes users (with password hashes) + groups
    pub styles: Vec<Style>,
}
```

`Auth` carries `config_dir`. When a client applies a snapshot, it overrides
`auth.config_dir` with its own local value so it does not inherit the owner's
paths.

### `ConfigSyncBackend` trait

Abstracts "where do I read version and data from":

```rust
#[async_trait]
trait ConfigSyncBackend: Send + Sync {
    async fn current_version(&self) -> AppResult<i64>;
    async fn fetch_snapshot(&self) -> AppResult<ConfigSnapshot>;
}
```

- **`LocalBackend { pool }`** (`shared`/`owner`): `current_version` =
  `get_config_version(pool)`; `fetch_snapshot` rebuilds from SQLite
  (`Catalog::new`, `get_categories`, `Auth::new`, `get_styles`).
- **`RemoteBackend { owner_url, secret, http }`** (`client`): `current_version`
  = `GET {owner}/internal/config/version`; `fetch_snapshot` =
  `GET {owner}/internal/config/snapshot`, deserialized from JSON.

### Watcher loop (single, generic over the backend)

```text
known = backend.current_version()          // at startup
loop:
    sleep(interval)
    current = backend.current_version()?    // on error: warn + continue
    if current > known:
        snapshot = backend.fetch_snapshot()?
        apply(snapshot)                     // swap under RwLocks: get_catalog /
                                            // get_categories / get_auth / get_styles_cache
        known = next_known_version(known, current, reload_ok)
```

`next_known_version(known, current, reload_ok)` advances `known` only when a
higher version was observed **and** the reload succeeded, so a failed reload is
retried on the next tick.

### Styles cache (new global)

Styles are now cached in memory like categories, in a new global
`STYLES: OnceCell<RwLock<Vec<Style>>>` with a `get_styles_cache()` getter
(mirroring `CATEGORIES`/`get_categories`). `Style` already derives
`Serialize`/`Deserialize`/`Clone`, and `get_styles(pool)` returns `Vec<Style>`.

The public read endpoints switch from SQLite to the in-memory cache so clients
(which have no SQLite) can serve them:

- `services::styles::index` (`/styles/{style_name}`) and `services::legends::index`
  (`/legends/{style_name}`) currently call `Style::from_category_and_name` →
  `get_style_by_category_and_name(None)` → `get_cf_pool()`. They are changed to
  look up the style by `category:name` in `get_styles_cache()` (in memory).
- The admin style pages (`html::admin::styles::*`) keep using the pool — they run
  only on the owner (routed there by nginx) and are write/list paths.

### Internal API (owner only)

A sub-router mounted on the same port, outside session auth, behind a
cluster-secret hoop:

- `GET /internal/config/version` → `{ "version": N }`
- `GET /internal/config/snapshot` → `ConfigSnapshot` as JSON

The hoop compares the `X-Cluster-Secret` header against `cluster.shared_secret`
with a constant-time comparison; mismatch → `401`.

## Configuration

New `ClusterConfig`:

```rust
#[derive(Debug, Deserialize, Default)]
pub struct ClusterConfig {
    #[serde(default = "default_cluster_mode")]
    pub mode: String,                       // standalone | shared | owner | client
    #[serde(default = "default_config_watch_interval")]
    pub config_watch_interval_secs: u64,    // default 10
    pub owner_url: Option<String>,          // required if mode = client
    pub shared_secret: Option<String>,      // required if mode = owner|client
}
```

Added to `Settings` as `#[serde(default)] pub cluster: ClusterConfig`, with
`set_default("cluster.mode", "standalone")` and
`set_default("cluster.config_watch_interval_secs", 10)`.

Env vars: `MVT_CLUSTER__MODE`, `MVT_CLUSTER__OWNER_URL`,
`MVT_CLUSTER__SHARED_SECRET`, `MVT_CLUSTER__CONFIG_WATCH_INTERVAL_SECS`.

### Validation (`Settings::validate()`)

- `mode` ∈ {standalone, shared, owner, client}; otherwise a clear error.
- `mode = client` → `owner_url` and `shared_secret` present and non-empty.
- `mode = owner` → `shared_secret` present and non-empty (recommend ≥ 16 chars).
- `mode = shared` / `standalone` → no extra requirements (the shared volume in
  situation 1 is a deployment choice).

## Startup wiring (`main.rs`)

Branch on `cluster.mode` to obtain `(catalog, categories, auth)`:

```text
if mode == client:
    snapshot = bootstrap_from_owner(owner_url, secret, http)   // retry w/ backoff
    catalog/categories/auth = from snapshot (auth.config_dir overridden)
    // NO init_sqlite, NO SQLITE_CONF.set(...)
else (standalone | shared | owner):
    cf_pool = init_sqlite(...)
    catalog/categories/auth = as today (from cf_pool)
    SQLITE_CONF.set(cf_pool)
```

`bootstrap_from_owner` retries with backoff (logging warnings) until it gets the
first snapshot, then proceeds to bind — so a client waits for the owner to be
ready (orchestrator-friendly).

The rest of the globals are set for all modes: `DB_REGISTRY` (clients also serve
tiles from the same PostGIS), `CACHE_WRAPPER` (initialized with the snapshot
catalog on clients), `CATALOG`/`CATEGORIES`/`AUTH`/`STYLES`.

Watcher spawn:

```text
shared => start_config_watcher(LocalBackend { pool }, interval, config_dir)
client => start_config_watcher(RemoteBackend { owner_url, secret, http }, interval, config_dir)
owner | standalone => (no watcher)
```

## Router by mode (`routes::app_router`)

- `standalone` / `shared` → current full router (admin + API + tiles). No
  internal API.
- `owner` → full router **plus** the `/internal/config/*` sub-router behind the
  cluster-secret hoop.
- `client` → **reduced router**: public read routes only (tiles + styles +
  legends + health) + the auth hoops (which validate session/JWT in memory,
  without touching SQLite). It does **not** mount admin, write API, or
  `/internal`.

Why the reduced router on clients: a client has no `SQLITE_CONF`, so any handler
calling `get_cf_pool()` would panic. Not mounting the admin/write routes means
those paths are never reached (and nginx never routes them there anyway).

**Verified:** the tile pipeline serves entirely from memory. `get_request_user`
(`src/services/utils.rs:209`) and `validate_user_groups`
(`src/services/utils.rs:249`) resolve against `get_auth()` (in memory) and tile
handlers read `get_catalog()` (in memory); none call `get_cf_pool()`. The public
styles/legends endpoints currently *do* read SQLite (`get_cf_pool()` via
`Style::from_category_and_name`); this design moves them to `get_styles_cache()`
(in memory) so they are safe to mount on clients.

## Security

The snapshot includes `Auth` **with password hashes**. This is required, not
optional: the tile path supports **Basic auth**, validated in memory against the
hashes (`get_user_by_authorization` → `validate_user` → `validate_psw`). Since
clients serve tiles, they need the hashes in memory. (Bearer/JWT uses
`jwt_secret` and session uses `session_secret`; only Basic auth needs hashes.)

Therefore hashes cannot be stripped, and protection rests on transport:

1. **Cluster secret** on `/internal` (constant-time compare → `401`).
2. **TLS mandatory, or a trusted private network** for situation 2 — the hashes
   cross hosts, so this is a requirement, not a recommendation. `reqwest` with
   `rustls` speaks HTTPS to the owner; alternative: WireGuard / private subnet.
3. **`/internal` not exposed publicly** by nginx.

Hashes are salted argon2 (strong even if leaked), but are treated as credentials
and never travel in cleartext.

## New dependency

`reqwest` with `rustls` (aligned with the project's `tls-rustls-aws-lc-rs` TLS
choice), used only by the `client` role for outbound calls to the owner. The
`standalone`/`shared` modes never exercise it.

## Testing strategy

TDD (failing test → implementation). Three levels:

**A. Unit tests** (in-memory SQLite pool via a shared `test_support` helper):

1. `config_version` `get`/`bump` (starts at 0, increments, persists).
2. Bumps on writes: categories/layers/users/groups/**styles** raise the version.
3. `next_known_version` pure logic (advances only when reload succeeded).
4. `cluster.*` validation: invalid `mode` fails; `client` without
   `owner_url`/`shared_secret` fails; `owner` without `shared_secret` fails;
   `standalone`/`shared` valid without extras.
5. `ConfigSnapshot` serialization round-trip (incl. `Auth` with hashes/groups and
   `styles`).
6. `LocalBackend`: `current_version` and `fetch_snapshot` against the in-memory
   pool return the expected catalog/categories/auth/**styles**.

**B. Integration tests** (`tests/integration/`, salvo `test` feature):

7. Internal-API secret hoop: `GET /internal/config/snapshot` without header or
   with wrong secret → `401`; with the correct secret → `200` + valid JSON.
8. `RemoteBackend` against a real server: start the owner internal router on an
   ephemeral port, point `RemoteBackend` (reqwest) at it, assert
   `current_version`/`fetch_snapshot` match the owner's `LocalBackend`.

**C. Manual verification** (touches global singletons / nginx):

9. Owner+client smoke test: owner on `:5887` (temp SQLite), client on `:5888`
   with `owner_url` + secret + short interval (3s).
   - Edit/create a layer on the owner → within ≤3s the client logs the reload
     and `GET :5888/api/catalog/layer` reflects the change without restart.
   - Edit a style on the owner → within ≤3s `GET :5888/styles/{cat:name}` on the
     client returns the updated style (served from the synced in-memory cache).
   - Basic auth on client tiles: request a protected tile from the client with
     `Authorization: Basic ...` → validates against the synced hashes.
   - `GET :5887/internal/config/snapshot` without the secret → `401`.

## Relationship to the paused plan

The earlier paused plan
(`docs/superpowers/plans/2026-06-22-multi-instance-config-sync.md`) implemented
situation 1 only (shared SQLite volume + version counter + watcher). Its tasks
remain valid and are reused here:

- `config_version` infra and the bump-on-write tasks are shared by both
  situations.
- `next_known_version` and the watcher loop are generalized over the
  `ConfigSyncBackend` trait.

This design adds situation 2 (owner/client + internal API + `RemoteBackend` +
cluster config + nginx routing) on top, additively. It also **supersedes the
paused plan's exclusion of styles**: the paused plan left styles out (not cached
in memory); here all five admin item types — users, groups, categories, catalog
layers, and styles — are cached, versioned, snapshotted, and reloaded uniformly,
which requires the new `STYLES` global and moving the public styles/legends read
path to memory.

## Out of scope

- Tile cache invalidation across instances (a layer deleted on the owner stays
  cached in Redis/disk until separately invalidated).
- Cross-host high availability of the owner (single writer; if the owner is down,
  clients keep serving from memory but config cannot be edited and new clients
  cannot cold-start).
- A local snapshot disk cache on clients for cold-start while the owner is down
  (possible future enhancement; not in v1).
- The orphan `src/plugins/watcher.rs` (left untouched).
