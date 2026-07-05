# Cluster Plugin Sync Design

**Date:** 2026-07-05
**Status:** Draft — pending user review
**Branch:** `feat/cluster-plugin-sync`

## Goal

Make Lua plugins work correctly in every cluster mode (`shared`, `owner`,
`client`), so that a plugin change made on one instance propagates to all
instances **without restarting them**, and all instances behind the load
balancer generate identical tiles.

Today plugins are loaded once at startup from the local `paths.plugins`
directory into an immutable `OnceLock<LuaPluginRegistry>`. The cluster
`ConfigSnapshot` does not carry them, so in a cluster each instance can serve
different plugin filters forever.

## Assumptions taken (user was away — please veto if wrong)

1. **Manual trigger.** Plugin changes are propagated by an explicit
   `POST /api/plugins/reload` (admin-only) plus a "Reload" button on the
   `/admin/plugins` page. No filesystem auto-watcher (no `notify` dependency).
2. **In `client` mode the local plugins directory is ignored.** The owner is
   the single source of truth; clients receive plugin sources over the
   existing internal HTTP API.
3. **The reload endpoint also works in `standalone`**, giving hot reload of
   plugins without a restart. This is purely additive.

## Backward compatibility (hard requirement)

With `cluster.mode = standalone` — explicit or implicit (no `cluster` block in
`config.yaml`) — behavior is unchanged:

- Startup still loads plugins from `paths.plugins` exactly as today.
- No watcher runs; no internal API is mounted (unchanged).
- The only additions are the reload endpoint/button (new capability, changes
  nothing unless called) and the registry being behind a `RwLock` (internal
  refactor, same semantics).
- Calling reload in standalone bumps `config_version` in SQLite; nothing
  listens to it there, so it is harmless.

## Approaches considered

- **A. Plugins ride the existing `ConfigSnapshot` / `config_version`**
  (chosen). One version counter, one watcher, one snapshot endpoint. Plugin
  sources (`.lua` text) are added to the snapshot; `apply_snapshot` rebuilds
  the registry. Cost: unrelated config edits also rebuild the Lua VMs —
  negligible (few small scripts).
- **B. Separate `plugins_version` channel** (direction of the orphan code in
  `src/plugins/watcher.rs` / `src/api/plugins.rs`). Independent cadence, but
  duplicates the watcher/endpoint/bootstrap machinery, and the orphan watcher
  polled SQLite, which does not exist in `client` mode. Rejected.
- **C. Store plugins in the SQLite config DB** with admin CRUD. Changes the
  operator workflow (files → DB rows); scope creep. Rejected.

## Architecture

Plugins become part of the **versioned config**. The snapshot is extended
with the plugin sources read from the owner's disk; every instance rebuilds
its in-memory `LuaPluginRegistry` from those sources when the snapshot is
applied.

```
owner/shared instance                          client instance
┌────────────────────────┐                     ┌───────────────────────┐
│ plugins/*.lua (disk)   │                     │ (local plugins dir    │
│        │ reload (admin)│                     │  ignored)             │
│        ▼               │  /internal/config/  │                       │
│ RwLock<LuaRegistry>    │  version + snapshot │ RwLock<LuaRegistry>   │
│ bump config_version ───┼────────────────────▶│ rebuilt from snapshot │
└────────────────────────┘   (plugins included)└───────────────────────┘
```

### Data model

```rust
// serializable plugin source, in src/plugins/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSource {
    pub key: String,     // file stem: "{category}" or "{category}_{layer}"
    pub source: String,  // full Lua source text
}
```

`ConfigSnapshot` gains:

```rust
#[serde(default)]              // tolerate snapshots from an older owner
pub plugins: Vec<PluginSource>,
```

### Registry changes (`src/plugins/mod.rs`)

- New constructor `LuaPluginRegistry::from_sources(Vec<PluginSource>) -> Self`.
  `new(plugins_dir)` becomes: read dir → collect `PluginSource`s →
  `from_sources`. The test-only `from_scripts` is replaced by `from_sources`.
- New method `read_sources(plugins_dir) -> Vec<PluginSource>` (used by
  `new` and by `build_snapshot`).
- The registry records load failures: `load_errors: Vec<PluginLoadError
  { key, error }>`, exposed for the reload response and the admin page.
  (Today failures are only logged.)

### Global state (`src/main.rs`)

- `PLUGIN_REGISTRY` changes from `OnceLock<LuaPluginRegistry>` to
  `OnceCell<RwLock<LuaPluginRegistry>>`, mirroring `CATALOG`/`AUTH`.
  `get_plugin_registry()` becomes `async fn -> &'static RwLock<LuaPluginRegistry>`.
- New `PLUGINS_DIR: OnceLock<String>` + `get_plugins_dir()` (set in the
  non-client branch; unset in client mode where disk is not used).
- Client branch: registry initialized with
  `LuaPluginRegistry::from_sources(snapshot.plugins)` instead of reading disk.

### Snapshot pipeline (`src/cluster/snapshot.rs`)

- `build_snapshot` additionally reads the plugin sources **from the plugins
  directory on disk** (`get_plugins_dir()`). Reading from disk (not from the
  in-memory registry) is what makes `shared` mode correct: a peer that never
  reloaded still builds a fresh snapshot from the shared volume.
- `apply_snapshot` also swaps the registry:
  `*get_plugin_registry().await.write().await = LuaPluginRegistry::from_sources(snapshot.plugins)`.

### Reload endpoint (`src/api/plugins.rs`, rewritten and wired)

`POST /api/plugins/reload`, mounted in `build_api_routes()` behind admin auth
(same guard as other admin APIs). Behavior:

1. Rebuild registry from `get_plugins_dir()`.
2. Swap it into the `RwLock`.
3. `bump_config_version(get_cf_pool())` — peers (shared) and clients (via
   owner endpoints) pick it up within `config_watch_interval_secs`.
4. Respond with JSON: loaded plugin keys, failed plugins with error text, new
   version.

The `/admin/plugins` page gains a "Reload" button that calls this endpoint
and shows the result (including load errors).

Note: this endpoint exists only in the full router; the reduced client router
does not mount it (clients have no admin, unchanged).

### Dead code removal

- Delete orphan `src/plugins/watcher.rs`.
- Remove `get_plugins_version`/`bump_plugins_version` and their
  `#[allow(dead_code)]` from `src/config/system_settings.rs`. The
  `plugins_version` row inserted by the old migration stays in place,
  harmless (no new migration).

### Call-site updates (registry now behind `RwLock`)

`src/services/tiles/builder.rs`, `src/services/tiles/handlers.rs`,
`src/html/admin/plugins.rs`: acquire the read guard once per request
(`let registry = get_plugin_registry().await.read().await;`) and use it for
`has_plugin` / `call_filter` / `list_plugins`. In `handlers.rs:286` the guard
is hoisted out of the `any(...)` closure.

## Data flow by mode

- **standalone:** startup from disk (unchanged). Optional manual reload
  swaps the registry in place. No propagation needed.
- **shared:** admin calls reload on any instance → that instance swaps its
  registry and bumps `config_version` in the shared SQLite → each peer's
  existing config watcher sees the bump, rebuilds the snapshot locally
  (plugins re-read from the shared volume) and swaps. Requirement: the
  plugins directory must live on the shared volume — documented.
- **owner:** reload on the owner → owner swaps + bumps. Clients poll
  `/internal/config/version`, fetch `/internal/config/snapshot` (now with
  plugin sources), and `apply_snapshot` rebuilds their registries.
- **client:** never reads local plugin files; registry comes from the
  bootstrap snapshot and subsequent watcher reloads.

## Consistency and caching

- Propagation window = `config_watch_interval_secs` (default 10 s), same as
  every other config change. During the window instances may serve different
  plugin filters; documented as expected behavior.
- **No cache invalidation is needed:** layers with an active plugin already
  bypass the server tile cache and client caching entirely
  (`builder.rs`, `handlers.rs`), so no stale plugin-filtered tile can be
  served from Redis. Adding/removing a plugin flips `has_plugin`, which
  switches the layer between cached/uncached paths safely.

## Error handling

- A `.lua` that fails to load is skipped with a warning (existing behavior);
  the reload response and admin page surface the error so a broken plugin is
  visible, and the remaining plugins still load.
- A snapshot from an older owner without the `plugins` field deserializes to
  an empty list (`serde(default)`); a client applying it simply runs with no
  plugins rather than failing.
- Failed snapshot fetch/apply keeps the previous registry (existing watcher
  retry semantics — known version does not advance).

## Security

Plugin sources are executable Lua distributed owner → clients. They travel
only over the existing `/internal/config/*` API, which already ships password
hashes and is guarded by the constant-time cluster-secret check. The docs
already require a strong secret and a private network / TLS for the internal
API; add an explicit note that this channel now also distributes code.

## Testing

- `from_sources` builds a working registry (reuse/port existing
  `from_scripts` tests).
- `read_sources` on a temp dir picks up `.lua` files and skips others.
- Snapshot: `build_snapshot` includes plugins from disk; JSON round-trip with
  plugins; deserializing legacy JSON without `plugins` yields empty vec.
- `apply_snapshot` swaps the registry (a `has_plugin` that was false becomes
  true).
- Reload handler: swaps registry, bumps version, reports load errors;
  rejected without admin auth.
- Registry load errors are collected, not just logged.
- Existing plugin tests keep passing after the `from_scripts` →
  `from_sources` refactor.

## Documentation updates

- `docs/clustering.md`: new "Plugins in a cluster" section — how sync works
  per mode, the reload endpoint/button, the shared-volume requirement in
  `shared` mode, the propagation window, the security note.
- `docs/plugins.md`: document `POST /api/plugins/reload` and hot reload;
  note cluster behavior and that client instances ignore their local plugins
  directory.
- `README.md`: extend the Lua plugin bullet ("hot reload, cluster-aware").

## Out of scope

- Editing plugins from the admin UI (they remain files on the owner's disk).
- Automatic filesystem watching (`notify`).
- A separate `plugins_version` channel.
