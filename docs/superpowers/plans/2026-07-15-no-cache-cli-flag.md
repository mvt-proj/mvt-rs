# `--no-cache` CLI Flag Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `--no-cache` CLI flag that disables tile caching (Redis or disk) entirely, for use in local dev and tests.

**Architecture:** A new `CacheMode::Disabled` variant in the existing `CacheWrapper`/`CacheMode` enum (`src/cache/cachewrapper.rs`) makes every cache operation a no-op. The flag is parsed by `clap` into `CliArgs.no_cache` and copied onto `Settings.no_cache` (CLI-only, not part of the `config` crate's YAML/env layering). `Settings.no_cache` is threaded into both call sites of `CacheWrapper::initialize_cache` in `src/main.rs`.

**Tech Stack:** Rust, clap (derive), tokio (async), existing `AppResult`/`AppError` error handling.

## Global Constraints

- CLI-only: no `MVT_...` env var, no `config.yaml` key for this setting (per design doc).
- No cache-clearing at startup when the flag is set — `initialize_cache` must skip Redis/disk entirely, not connect-then-ignore.
- Existing behavior (Redis/disk mode) must be unchanged when the flag is absent.

---

### Task 1: CLI flag and `Settings.no_cache`

**Files:**
- Modify: `src/config/settings.rs`
- Test: `src/config/settings.rs` (existing `#[cfg(test)] mod tests` block at the bottom of the file)

**Interfaces:**
- Produces: `CliArgs.no_cache: bool` (clap flag, default `false`), `Settings.no_cache: bool` (default `false`, set from CLI in `Settings::new()`).
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block at the bottom of `src/config/settings.rs` (after the existing `standalone_does_not_require_redis` test):

```rust
    #[test]
    fn no_cache_flag_defaults_to_false() {
        let args = CliArgs::parse_from(["mvt-rs"]);
        assert!(!args.no_cache);
    }

    #[test]
    fn no_cache_flag_parses_true() {
        let args = CliArgs::parse_from(["mvt-rs", "--no-cache"]);
        assert!(args.no_cache);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::settings::tests::no_cache_flag -- --nocapture`
Expected: FAIL to compile — `no field 'no_cache' on type 'CliArgs'`.

- [ ] **Step 3: Implement the CLI flag and Settings field**

In `src/config/settings.rs`, add the flag to `CliArgs` (after the `port` field):

```rust
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct CliArgs {
    #[arg(short, long)]
    pub config: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub no_cache: bool,
}
```

Add the field to `Settings` (it must NOT come from the `config` crate's file/env layering, hence `#[serde(skip)]`):

```rust
#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    #[serde(default)] pub server: ServerConfig,
    #[serde(default)] pub database: DatabaseConfig,
    #[serde(default)] pub postgres_databases: PostgresDatabasesConfig,
    #[serde(default)] pub security: SecurityConfig,
    #[serde(default)] pub paths: PathConfig,
    #[serde(default)] pub cluster: ClusterConfig,
    #[serde(skip)] pub no_cache: bool,
}
```

In `Settings::new()`, capture the flag right after parsing args (bool is `Copy`, so this doesn't interfere with the later partial moves of `args.config`/`args.host`/`args.port`):

```rust
    pub fn new() -> Result<Self, config::ConfigError> {
        let args = CliArgs::parse();
        let no_cache = args.no_cache;
        let config_path = args
            .config
            .unwrap_or_else(|| "config/config.yaml".to_string());
```

And apply it after deserialization, changing `let settings` to `let mut settings`:

```rust
        let mut settings: Settings = s.try_deserialize().map_err(|e| {
            tracing::error!("Error deserializing configuration: {}", e);
            e
        })?;
        settings.no_cache = no_cache;

        tracing::debug!("Loaded settings: {:?}", settings);

        Ok(settings)
    }
```

Finally, fix the `valid_settings()` test helper (it builds a `Settings` literal, which now requires the new field) — add `no_cache: false,` after the `cluster:` field in `src/config/settings.rs`'s `mod tests`:

```rust
            cluster: ClusterConfig {
                mode: "standalone".to_string(),
                config_watch_interval_secs: 10,
                cache_invalidation_extra_delay_secs: 5,
                owner_url: None,
                shared_secret: None,
            },
            no_cache: false,
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::settings`
Expected: PASS — all tests in `config::settings::tests`, including the two new ones.

- [ ] **Step 5: Commit**

```bash
git add src/config/settings.rs
git commit -m "feat(config): add --no-cache CLI flag"
```

---

### Task 2: `CacheMode::Disabled` and wiring into `main.rs`

**Files:**
- Modify: `src/cache/cachewrapper.rs`
- Modify: `src/main.rs:240-245` and `src/main.rs:287-292`
- Test: `src/cache/cachewrapper.rs` (new `#[cfg(test)] mod tests` block)

**Interfaces:**
- Consumes: `Settings.no_cache: bool` (Task 1).
- Produces: `CacheMode::Disabled` variant, `CacheWrapper::new_disabled() -> CacheWrapper`, updated signature `CacheWrapper::initialize_cache(redis_conn: Option<String>, disk_cache_dir: PathBuf, catalog: Catalog, disabled: bool) -> AppResult<CacheWrapper>`.

- [ ] **Step 1: Write the failing tests**

Add to the bottom of `src/cache/cachewrapper.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn empty_catalog() -> Catalog {
        Catalog { layers: vec![] }
    }

    #[tokio::test]
    async fn disabled_mode_write_then_get_returns_none() {
        let wrapper = CacheWrapper::new_disabled();
        wrapper
            .write_tile("layer", 1, 2, 3, b"tile-bytes", 0)
            .await
            .expect("write_tile should be a no-op success");

        let tile = wrapper.get_tile("layer", 1, 2, 3, 0).await;
        assert!(tile.is_none());
    }

    #[tokio::test]
    async fn disabled_mode_delete_and_version_are_noop() {
        let wrapper = CacheWrapper::new_disabled();

        wrapper
            .delete_cache(empty_catalog())
            .await
            .expect("delete_cache no-op");
        wrapper
            .delete_layer_cache(&"layer".to_string())
            .await
            .expect("delete_layer_cache no-op");

        assert_eq!(wrapper.get_layer_version("layer").await, 0);
        wrapper.increment_layer_version("layer").await;
        assert_eq!(wrapper.get_layer_version("layer").await, 0);

        assert!(!wrapper.exists_key("key".to_string()).await.unwrap());
        assert_eq!(wrapper.cache_dir(), PathBuf::new());
    }

    #[tokio::test]
    async fn initialize_cache_disabled_skips_disk_setup() {
        let untouched_dir =
            std::env::temp_dir().join("mvt-rs-test-no-cache-untouched");

        let wrapper = CacheWrapper::initialize_cache(
            None,
            untouched_dir.clone(),
            empty_catalog(),
            true,
        )
        .await
        .expect("disabled cache should initialize without a backend");

        assert_eq!(wrapper.cache_dir(), PathBuf::new());
        assert!(!untouched_dir.exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib cache::cachewrapper::tests -- --nocapture`
Expected: FAIL to compile — `no function or associated item named 'new_disabled' found for struct 'CacheWrapper'` and `this function takes 4 arguments but 3 arguments were supplied` (or similar, since `initialize_cache` doesn't yet take a `disabled` parameter).

- [ ] **Step 3: Implement `CacheMode::Disabled`**

In `src/cache/cachewrapper.rs`, add the variant:

```rust
#[derive(Debug, Clone)]
pub enum CacheMode {
    Redis(RedisCache),
    Disk(DiskCache),
    Disabled,
}
```

Add the constructor (next to `new_redis`/`new_disk`):

```rust
    pub fn new_disabled() -> Self {
        CacheWrapper {
            mode: CacheMode::Disabled,
        }
    }
```

Change `initialize_cache` to take the flag and short-circuit before touching Redis/disk:

```rust
    pub async fn initialize_cache(
        redis_conn: Option<String>,
        disk_cache_dir: PathBuf,
        catalog: Catalog,
        disabled: bool,
    ) -> AppResult<CacheWrapper> {
        if disabled {
            return Ok(CacheWrapper::new_disabled());
        }

        if let Some(redis_conn) = redis_conn
            && !redis_conn.is_empty()
        {
            let redis_cache = RedisCache::new(redis_conn).await?;
            redis_cache.delete_cache(catalog.clone()).await?;
            return Ok(CacheWrapper::new_redis(redis_cache));
        }

        let disk_cache = DiskCache::new(disk_cache_dir);
        disk_cache.delete_cache_dir(catalog).await;
        Ok(CacheWrapper::new_disk(disk_cache))
    }
```

Update `cache_dir`:

```rust
    pub fn cache_dir(&self) -> PathBuf {
        match &self.mode {
            CacheMode::Disk(disk_cache) => disk_cache.cache_dir.clone(),
            CacheMode::Redis(_) => PathBuf::new(),
            CacheMode::Disabled => PathBuf::new(),
        }
    }
```

Update `delete_cache` (short-circuit before the version-increment loop, since there is nothing to invalidate when nothing is ever cached):

```rust
    pub async fn delete_cache(&self, catalog: Catalog) -> AppResult<()> {
        if matches!(self.mode, CacheMode::Disabled) {
            return Ok(());
        }
        // Increment version for affected layers before clearing tiles, so any
        // in-flight requests that complete after this point get a fresh ETag.
        for layer in catalog.layers.iter() {
            if layer.delete_cache_on_start.unwrap_or(false) {
                let key = format!("{}_{}", layer.category.name, layer.name);
                self.increment_layer_version(&key).await;
            }
        }
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.delete_cache(catalog).await,
            CacheMode::Disk(disk_cache) => {
                disk_cache.delete_cache_dir(catalog).await;
                Ok(())
            }
            CacheMode::Disabled => Ok(()),
        }
    }
```

Update `delete_layer_cache`:

```rust
    pub async fn delete_layer_cache(&self, layer_name: &String) -> AppResult<()> {
        if matches!(self.mode, CacheMode::Disabled) {
            return Ok(());
        }
        self.increment_layer_version(layer_name).await;
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.delete_layer_cache(layer_name).await,
            CacheMode::Disk(disk_cache) => {
                disk_cache.delete_layer_cache(layer_name).await;
                Ok(())
            }
            CacheMode::Disabled => Ok(()),
        }
    }
```

Update `get_layer_version`:

```rust
    pub async fn get_layer_version(&self, layer_name: &str) -> u64 {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.get_layer_version(layer_name).await,
            CacheMode::Disk(disk_cache) => disk_cache.get_layer_version(layer_name).await,
            CacheMode::Disabled => 0,
        }
    }
```

Update `increment_layer_version`:

```rust
    pub async fn increment_layer_version(&self, layer_name: &str) {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.increment_layer_version(layer_name).await,
            CacheMode::Disk(disk_cache) => disk_cache.increment_layer_version(layer_name).await,
            CacheMode::Disabled => {}
        }
    }
```

Update `get_tile`:

```rust
    pub async fn get_tile(
        &self,
        name: &str,
        z: u32,
        x: u32,
        y: u32,
        max_cache_age: u64,
    ) -> Option<Bytes> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => {
                let key = format!("{name}:{z}:{x}:{y}");
                redis_cache.get_cache(key).await.ok()
            }
            CacheMode::Disk(disk_cache) => {
                let tilefolder = disk_cache
                    .cache_dir
                    .join(name)
                    .join(z.to_string())
                    .join(x.to_string());
                let tilepath = tilefolder.join(y.to_string()).with_extension("pbf");
                disk_cache.get_cache(tilepath, max_cache_age).await.ok()
            }
            CacheMode::Disabled => None,
        }
    }
```

Update `write_tile`:

```rust
    pub async fn write_tile(
        &self,
        name: &str,
        z: u32,
        x: u32,
        y: u32,
        tile: &[u8],
        max_cache_age: u64,
    ) -> AppResult<()> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => {
                let key = format!("{name}:{z}:{x}:{y}");
                redis_cache
                    .write_tile_to_cache(key, tile, max_cache_age)
                    .await
            }
            CacheMode::Disk(disk_cache) => {
                let tilefolder = disk_cache
                    .cache_dir
                    .join(name)
                    .join(z.to_string())
                    .join(x.to_string());
                let tilepath = tilefolder.join(y.to_string()).with_extension("pbf");
                disk_cache.write_tile_to_file(&tilepath, tile).await
            }
            CacheMode::Disabled => Ok(()),
        }
    }
```

Update `exists_key`:

```rust
    pub async fn exists_key(&self, key: String) -> AppResult<bool> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.exists_key(key).await,
            CacheMode::Disk(_) => Ok(false),
            CacheMode::Disabled => Ok(false),
        }
    }
```

- [ ] **Step 4: Wire `settings.no_cache` into both `main.rs` call sites**

In `src/main.rs`, the cluster-client branch (~line 240):

```rust
        let cache_wrapper = CacheWrapper::initialize_cache(
            settings.database.redis_url.clone(),
            settings.paths.cache.clone().into(),
            snapshot.catalog.clone(),
            settings.no_cache,
        )
        .await?;
```

And the standalone/shared/owner branch (~line 287):

```rust
        let cache_wrapper = CacheWrapper::initialize_cache(
            settings.database.redis_url.clone(),
            settings.paths.cache.clone().into(),
            catalog.clone(),
            settings.no_cache,
        )
        .await?;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib cache::cachewrapper`
Expected: PASS — all three new tests in `cache::cachewrapper::tests`.

- [ ] **Step 6: Full build and test suite check**

Run: `cargo build && cargo test --lib`
Expected: builds cleanly, all existing and new tests pass (no regressions from the `initialize_cache` signature change).

- [ ] **Step 7: Commit**

```bash
git add src/cache/cachewrapper.rs src/main.rs
git commit -m "feat(cache): add CacheMode::Disabled for --no-cache"
```

---

## Manual Verification (not automated)

After Task 2 lands, confirm end-to-end behavior once:

```bash
cargo run -- --no-cache
```

Expected in the startup logs (from the existing `tracing::debug!("Loaded settings: {:?}", settings)` line): `no_cache: true`. Request the same tile twice; both responses should be freshly generated (no `X-Cache: HIT`-style behavior — check `src/services/tiles/handlers.rs` cache-hit logging/headers if present) rather than served from a cached file/Redis key.
