# Multi-Instance Clustering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the in-memory config (`Catalog`, `Categories`, `Auth`, `Styles`) fresh across multiple `mvt-rs` instances behind a load balancer, both same-host (shared SQLite volume) and cross-host (one owner instance serving config to clients over an internal HTTP API).

**Architecture:** A single SQLite holds config. A `config_version` counter in `system_settings` is bumped on every config write. Each non-owner instance runs a watcher that, when the version rises, rebuilds the four in-memory states. The watcher reads version+data either locally (shared SQLite) or from the owner's authenticated internal API (cross-host). Mode is chosen by `cluster.mode`.

**Tech Stack:** Rust, SQLx 0.9 (SQLite), Salvo 0.93, Tokio, `reqwest` (rustls), Askama. Migrations via `sqlx::migrate!`.

## Global Constraints

- A single SQLite database for config — no Postgres/Redis as a config store, no per-host SQLite copies, no network filesystem.
- Default behavior must be unchanged: absent `cluster` block ⇒ `mode = standalone` ⇒ exactly today's single-instance behavior.
- All five admin item types (users, groups, categories, catalog layers, styles) are treated uniformly: cached in memory, bump `config_version` on write, travel in the snapshot, rebuilt by the watcher.
- Config-write functions in `src/config/*.rs` return `Result<_, sqlx::Error>`; the version helpers must too, so `?` composes. `AppError` already implements `From<sqlx::Error>`.
- The snapshot includes `Auth` **with** password hashes (clients need them for Basic-auth tile validation). Protect it with a cluster secret + TLS/private network; never expose `/internal` publicly.
- Required secrets keep their existing rule: `jwt_secret`/`session_secret` ≥ 32 chars (unchanged). New `cluster.shared_secret` recommended ≥ 16 chars.
- New external dependency `reqwest` is used only by the `client` role.
- Leave the orphan `src/plugins/watcher.rs` untouched.

---

### Task 1: `config_version` counter + shared test pool

**Files:**
- Create: `migrations/20260623120000_config_version.sql`
- Modify: `src/config/system_settings.rs` (add two functions + tests)
- Create: `src/config/test_support.rs`
- Modify: `src/config/mod.rs` (register `test_support` under `cfg(test)`)

**Interfaces:**
- Produces:
  - `config::system_settings::get_config_version(pool: &SqlitePool) -> Result<i64, sqlx::Error>`
  - `config::system_settings::bump_config_version(pool: &SqlitePool) -> Result<i64, sqlx::Error>` (returns the new value)
  - `config::test_support::in_memory_pool() -> sqlx::SqlitePool` (cfg(test) only)

- [ ] **Step 1: Add the migration**

Create `migrations/20260623120000_config_version.sql`:

```sql
-- Global config version counter for multi-instance in-memory state sync.
-- Bumped on every config write; polled by the config watcher in each instance.
INSERT OR IGNORE INTO system_settings (key, value) VALUES ('config_version', '0');
```

- [ ] **Step 2: Create the shared test pool helper**

Create `src/config/test_support.rs`:

```rust
#![cfg(test)]

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

/// In-memory SQLite pool with all migrations applied. `max_connections(1)` is
/// required: each connection to `sqlite::memory:` gets its own database, so a
/// multi-connection pool would migrate one DB and query another.
pub async fn in_memory_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}
```

- [ ] **Step 3: Register the test module**

In `src/config/mod.rs`, add after the existing `pub mod` lines:

```rust
#[cfg(test)]
mod test_support;
```

- [ ] **Step 4: Write the failing tests**

Append to `src/config/system_settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn config_version_starts_at_zero() {
        let pool = in_memory_pool().await;
        assert_eq!(get_config_version(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn bump_increments_and_returns_new_value() {
        let pool = in_memory_pool().await;
        assert_eq!(bump_config_version(&pool).await.unwrap(), 1);
        assert_eq!(bump_config_version(&pool).await.unwrap(), 2);
        assert_eq!(get_config_version(&pool).await.unwrap(), 2);
    }
}
```

- [ ] **Step 5: Run tests to verify they fail**

Run: `cargo test --lib config::system_settings`
Expected: FAIL — `cannot find function get_config_version` / `bump_config_version`.

- [ ] **Step 6: Implement the helpers**

Append to `src/config/system_settings.rs` (before the `#[cfg(test)]` module):

```rust
pub async fn get_config_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (String,) =
        sqlx::query_as("SELECT value FROM system_settings WHERE key = 'config_version'")
            .fetch_one(pool)
            .await?;
    Ok(row.0.parse().unwrap_or(0))
}

pub async fn bump_config_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let new_version: (i64,) = sqlx::query_as(
        "UPDATE system_settings SET value = CAST(value AS INTEGER) + 1
         WHERE key = 'config_version'
         RETURNING CAST(value AS INTEGER)",
    )
    .fetch_one(pool)
    .await?;
    Ok(new_version.0)
}
```

Note: `system_settings.rs` already imports `use sqlx::SqlitePool;`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --lib config::system_settings`
Expected: PASS (2 tests).

- [ ] **Step 8: Commit**

```bash
git add migrations/20260623120000_config_version.sql src/config/system_settings.rs src/config/test_support.rs src/config/mod.rs
git commit -m "feat(config): add config_version counter and shared test pool"
```

---

### Task 2: Bump `config_version` on category writes

**Files:**
- Modify: `src/config/categories.rs` (3 write functions + tests)

**Interfaces:**
- Consumes: `bump_config_version` (Task 1).
- Produces: `create_category`/`update_category`/`delete_category` bump the version (signatures unchanged).

- [ ] **Step 1: Write the failing tests**

Append to `src/config/categories.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;

    fn cat(id: &str) -> Category {
        Category { id: id.into(), name: format!("name-{id}"), description: "d".into() }
    }

    #[tokio::test]
    async fn create_category_bumps_version() {
        let pool = in_memory_pool().await;
        create_category(Some(&pool), cat("c1")).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn update_category_bumps_version() {
        let pool = in_memory_pool().await;
        create_category(Some(&pool), cat("c1")).await.unwrap();
        update_category(Some(&pool), cat("c1")).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn delete_category_bumps_version() {
        let pool = in_memory_pool().await;
        delete_category(Some(&pool), "nonexistent").await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
```

Note: confirm the `create_category`/`update_category` argument order matches the file (`Some(&pool), category`). Adjust the calls if the actual signature differs.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::categories`
Expected: FAIL — version is 0, asserts expect 1/2/1.

- [ ] **Step 3: Add the import**

At the top of `src/config/categories.rs`, after `use crate::get_cf_pool;`:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to all three functions**

In each of `create_category`, `update_category`, `delete_category`, insert `bump_config_version(pool).await?;` immediately after the final `.execute(pool).await?;` and before `Ok(())`. Example for `delete_category`:

```rust
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib config::categories`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/config/categories.rs
git commit -m "feat(config): bump config_version on category writes"
```

---

### Task 3: Bump `config_version` on layer writes

**Files:**
- Modify: `src/config/layers.rs` (`create_layer`, `update_layer`, `delete_layer`, `switch_layer_published` + test)

**Interfaces:**
- Consumes: `bump_config_version` (Task 1).
- Produces: all four layer write functions bump the version (signatures unchanged).

- [ ] **Step 1: Write the failing test**

`delete_layer(pool, id)` needs no row, so it is the clean seam. Append to `src/config/layers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn delete_layer_bumps_version() {
        let pool = in_memory_pool().await;
        delete_layer(Some(&pool), "nonexistent").await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::layers`
Expected: FAIL — version 0, expected 1.

- [ ] **Step 3: Add the import**

At the top of `src/config/layers.rs`, after `use crate::get_cf_pool;`:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to all four write functions**

In `create_layer`, `update_layer`, `delete_layer`, and `switch_layer_published`, insert `bump_config_version(pool).await?;` immediately after the final `.execute(pool).await?;` and before `Ok(())`. For `switch_layer_published` it goes after the `UPDATE` execute (not the `SELECT`).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib config::layers`
Expected: PASS (1 test).

- [ ] **Step 6: Commit**

```bash
git add src/config/layers.rs
git commit -m "feat(config): bump config_version on layer writes"
```

---

### Task 4: Bump `config_version` on user and group writes

**Files:**
- Modify: `src/config/users.rs` (`create_user`, `update_user`, `delete_user` + test)
- Modify: `src/config/groups.rs` (`create_group`, `update_group`, `delete_group` + test)

**Interfaces:**
- Consumes: `bump_config_version` (Task 1).
- Produces: all user/group write functions bump the version (signatures unchanged).

- [ ] **Step 1: Write the failing tests**

Append to `src/config/users.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn delete_user_bumps_version() {
        let pool = in_memory_pool().await;
        delete_user("nonexistent".to_string(), Some(&pool)).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
```

Append to `src/config/groups.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn delete_group_bumps_version() {
        let pool = in_memory_pool().await;
        delete_group("nonexistent".to_string(), Some(&pool)).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::users config::groups`
Expected: FAIL — version 0, expected 1.

- [ ] **Step 3: Add the imports**

At the top of both `src/config/users.rs` and `src/config/groups.rs`, after `use crate::get_cf_pool;`:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to all six write functions**

In each of `create_user`, `update_user`, `delete_user`, `create_group`, `update_group`, `delete_group`, insert `bump_config_version(pool).await?;` immediately after the `.execute(pool).await?;` and before `Ok(())`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib config::users config::groups`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add src/config/users.rs src/config/groups.rs
git commit -m "feat(config): bump config_version on user and group writes"
```

---

### Task 5: Bump `config_version` on style writes

**Files:**
- Modify: `src/config/styles.rs` (`create_style`, `update_style`, `delete_style` + test)

**Interfaces:**
- Consumes: `bump_config_version` (Task 1).
- Produces: all three style write functions bump the version (signatures unchanged).

- [ ] **Step 1: Write the failing test**

`delete_style(id, pool)` needs no row. Append to `src/config/styles.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn delete_style_bumps_version() {
        let pool = in_memory_pool().await;
        delete_style("nonexistent", Some(&pool)).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::styles`
Expected: FAIL — version 0, expected 1.

- [ ] **Step 3: Add the import**

At the top of `src/config/styles.rs`, after `use crate::get_cf_pool;`:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to all three write functions**

In `create_style`, `update_style`, `delete_style`, insert `bump_config_version(pool).await?;` immediately after the `.execute(pool).await?;` and before `Ok(())`. Example for `delete_style`:

```rust
    .bind(id)
    .execute(pool)
    .await?;

    bump_config_version(pool).await?;

    Ok(())
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib config::styles`
Expected: PASS (1 test).

- [ ] **Step 6: Commit**

```bash
git add src/config/styles.rs
git commit -m "feat(config): bump config_version on style writes"
```

---

### Task 6: Cache styles in memory (`STYLES` global) + serve public styles/legends from memory

**Files:**
- Modify: `src/main.rs` (new `STYLES` global, `get_styles_cache`, `reload_styles_cache`, load at startup)
- Modify: `src/models/styles.rs` (pure `find_style` helper + `from_category_and_name_cached` + test)
- Modify: `src/services/styles.rs` (read from cache)
- Modify: `src/services/legends.rs` (read from cache)
- Modify: `src/html/admin/styles.rs` (inline cache refresh after writes)

**Interfaces:**
- Produces:
  - `crate::get_styles_cache() -> &'static RwLock<Vec<Style>>` (async getter)
  - `crate::reload_styles_cache() -> AppResult<()>`
  - `models::styles::find_style<'a>(styles: &'a [Style], category: &str, name: &str) -> Option<&'a Style>`
  - `Style::from_category_and_name_cached(category: &str, name: &str) -> AppResult<Style>`

- [ ] **Step 1: Write the failing test for the pure lookup**

Append to `src/models/styles.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::category::Category;

    fn style(cat: &str, name: &str) -> Style {
        Style {
            id: format!("{cat}-{name}"),
            name: name.into(),
            category: Category { id: cat.into(), name: cat.into(), description: String::new() },
            description: String::new(),
            style: "{}".into(),
        }
    }

    #[test]
    fn find_style_matches_category_name_and_style_name() {
        let styles = vec![style("roads", "default"), style("water", "blue")];
        assert_eq!(find_style(&styles, "water", "blue").unwrap().id, "water-blue");
        assert!(find_style(&styles, "water", "default").is_none());
        assert!(find_style(&styles, "nope", "blue").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib models::styles`
Expected: FAIL — `cannot find function find_style`.

- [ ] **Step 3: Add the `STYLES` global and helpers in `main.rs`**

In `src/main.rs`, add to the imports near `use models::{catalog::Catalog, category::Category};`:

```rust
use models::styles::Style;
```

After the `AUTH` global block (after the `get_auth` fn, around line 86), add:

```rust
static STYLES: OnceCell<RwLock<Vec<Style>>> = OnceCell::const_new();
#[inline]
pub async fn get_styles_cache() -> &'static RwLock<Vec<Style>> {
    STYLES.get().unwrap()
}

pub async fn reload_styles_cache() -> AppResult<()> {
    let styles = config::styles::get_styles(Some(get_cf_pool())).await?;
    *get_styles_cache().await.write().await = styles;
    Ok(())
}
```

- [ ] **Step 4: Load styles into the global at startup**

In `src/main.rs`, after the `let categories = get_cf_categories(Some(&cf_pool)).await?;` line, add:

```rust
    let styles = config::styles::get_styles(Some(&cf_pool)).await?;
```

And after `AUTH.set(RwLock::new(auth)).unwrap();` add:

```rust
    STYLES.set(RwLock::new(styles)).unwrap();
```

- [ ] **Step 5: Add the pure helper + cached lookup in `models/styles.rs`**

In `src/models/styles.rs`, change the imports line:

```rust
use crate::{
    config::styles::{
        create_style, delete_style, get_style, get_style_by_category_and_name, get_styles,
        update_style,
    },
    error::{AppError, AppResult},
    models::category::Category,
};
```

Add a free function at the end of the file (outside `impl Style`, before the `#[cfg(test)]` module):

```rust
/// Finds a style in a slice by its category name and style name. Pure so it can
/// be unit-tested without the global cache.
pub fn find_style<'a>(styles: &'a [Style], category: &str, name: &str) -> Option<&'a Style> {
    styles.iter().find(|s| s.category.name == category && s.name == name)
}
```

Add this method inside `impl Style` (next to `from_category_and_name`):

```rust
    /// Reads from the in-memory styles cache (used by the public endpoints so
    /// instances without a SQLite — clients — can serve styles/legends).
    pub async fn from_category_and_name_cached(category: &str, name: &str) -> AppResult<Self> {
        let styles = crate::get_styles_cache().await.read().await;
        find_style(&styles, category, name)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("style {category}:{name}")))
    }
```

- [ ] **Step 6: Run the pure-lookup test to verify it passes**

Run: `cargo test --lib models::styles`
Expected: PASS.

- [ ] **Step 7: Switch the public endpoints to the cache**

In `src/services/styles.rs`, change line 12 from:

```rust
    let style = Style::from_category_and_name(category, name).await?;
```

to:

```rust
    let style = Style::from_category_and_name_cached(category, name).await?;
```

In `src/services/legends.rs`, change line 21 the same way.

- [ ] **Step 8: Add inline cache refresh after admin style writes**

In `src/html/admin/styles.rs`, in `create_style`, `update_style`, and `delete_style`, immediately before the `res.render(Redirect::other("/admin/styles"));` line (after the write succeeded), add:

```rust
    crate::reload_styles_cache().await?;
```

- [ ] **Step 9: Build and run the full test suite**

Run: `cargo build`
Expected: compiles.

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add src/main.rs src/models/styles.rs src/services/styles.rs src/services/legends.rs src/html/admin/styles.rs
git commit -m "feat(styles): cache styles in memory; serve public styles/legends from cache"
```

---

### Task 7: `ClusterConfig` settings + validation

**Files:**
- Modify: `src/config/settings.rs` (new `ClusterConfig`, defaults, `Settings` field, builder defaults, validation, tests)

**Interfaces:**
- Produces:
  - `config::settings::ClusterConfig { mode: String, config_watch_interval_secs: u64, owner_url: Option<String>, shared_secret: Option<String> }`
  - `Settings.cluster: ClusterConfig`
  - `default_cluster_mode() -> String`, `default_config_watch_interval() -> u64`

- [ ] **Step 1: Write the failing tests**

In `src/config/settings.rs`, inside the existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn cluster_mode_default_is_standalone() {
        assert_eq!(default_cluster_mode(), "standalone");
    }

    #[test]
    fn config_watch_interval_default_is_ten() {
        assert_eq!(default_config_watch_interval(), 10);
    }

    #[test]
    fn invalid_cluster_mode_fails() {
        let mut s = valid_settings();
        s.cluster.mode = "bogus".to_string();
        assert!(s.validate().is_err());
    }

    #[test]
    fn client_requires_owner_url_and_secret() {
        let mut s = valid_settings();
        s.cluster.mode = "client".to_string();
        assert!(s.validate().is_err());
        s.cluster.owner_url = Some("https://owner:5887".to_string());
        s.cluster.shared_secret = Some("a-cluster-secret-value".to_string());
        assert!(s.validate().is_ok());
    }

    #[test]
    fn owner_requires_secret() {
        let mut s = valid_settings();
        s.cluster.mode = "owner".to_string();
        assert!(s.validate().is_err());
        s.cluster.shared_secret = Some("a-cluster-secret-value".to_string());
        assert!(s.validate().is_ok());
    }
```

Note: `valid_settings()` is the existing test helper that builds a `Settings` with struct literals. Because `ClusterConfig` derives `Default`, update that helper to set `cluster: ClusterConfig::default()` (Step 4).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::settings`
Expected: FAIL — `cannot find function default_cluster_mode` and missing `cluster` field.

- [ ] **Step 3: Add `ClusterConfig`, defaults, and the `Settings` field**

In `src/config/settings.rs`, after the `ServerConfig` block and its `default_*` fns, add:

```rust
#[derive(Debug, Deserialize, Default)]
pub struct ClusterConfig {
    #[serde(default = "default_cluster_mode")]
    pub mode: String,
    #[serde(default = "default_config_watch_interval")]
    pub config_watch_interval_secs: u64,
    pub owner_url: Option<String>,
    pub shared_secret: Option<String>,
}

fn default_cluster_mode() -> String { "standalone".to_string() }
fn default_config_watch_interval() -> u64 { 10 }
```

In `struct Settings`, add the field:

```rust
    #[serde(default)] pub cluster: ClusterConfig,
```

In `Settings::new`, add to the builder chain (next to the other `set_default` calls):

```rust
            .set_default("cluster.mode", "standalone")?
            .set_default("cluster.config_watch_interval_secs", 10)?
```

- [ ] **Step 4: Add validation and fix the test helper**

In `Settings::validate()`, before the final `Ok(())`, add:

```rust
        match self.cluster.mode.as_str() {
            "standalone" | "shared" => {}
            "owner" => {
                if self.cluster.shared_secret.as_deref().unwrap_or("").is_empty() {
                    return Err("Configuration error: cluster.mode = owner requires \
                        cluster.shared_secret".to_string());
                }
            }
            "client" => {
                if self.cluster.owner_url.as_deref().unwrap_or("").is_empty() {
                    return Err("Configuration error: cluster.mode = client requires \
                        cluster.owner_url".to_string());
                }
                if self.cluster.shared_secret.as_deref().unwrap_or("").is_empty() {
                    return Err("Configuration error: cluster.mode = client requires \
                        cluster.shared_secret".to_string());
                }
            }
            other => {
                return Err(format!(
                    "Configuration error: invalid cluster.mode '{other}' \
                     (expected standalone | shared | owner | client)"
                ));
            }
        }
```

In the `valid_settings()` test helper, add the field to the `Settings { .. }` literal:

```rust
            cluster: ClusterConfig::default(),
```

`ClusterConfig::default()` yields `mode = ""`. Since `valid_settings()` must validate OK, set the mode explicitly in the helper right after constructing, or build the literal with `ClusterConfig { mode: "standalone".to_string(), config_watch_interval_secs: 10, owner_url: None, shared_secret: None }`. Use the explicit literal to keep `valid_settings()` passing.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib config::settings`
Expected: PASS (new + existing tests).

- [ ] **Step 6: Commit**

```bash
git add src/config/settings.rs
git commit -m "feat(config): add cluster settings (mode/interval/owner_url/shared_secret) + validation"
```

---

### Task 8: `cluster` module + `ConfigSnapshot` (build + apply)

**Files:**
- Create: `src/cluster/mod.rs`
- Create: `src/cluster/snapshot.rs`
- Modify: `src/main.rs` (add `mod cluster;`)

**Interfaces:**
- Consumes: `Catalog::new`, `config::categories::get_categories`, `Auth::new`, `config::styles::get_styles`, and the globals `get_catalog`/`get_categories`/`get_auth`/`get_styles_cache`.
- Produces:
  - `cluster::snapshot::ConfigSnapshot { catalog: Catalog, categories: Vec<Category>, auth: Auth, styles: Vec<Style> }` (Serialize/Deserialize)
  - `cluster::snapshot::build_snapshot(config_dir: &str, pool: &SqlitePool) -> AppResult<ConfigSnapshot>`
  - `cluster::snapshot::apply_snapshot(snapshot: ConfigSnapshot, config_dir: &str)`

- [ ] **Step 1: Create the module and register it**

Create `src/cluster/mod.rs`:

```rust
pub mod snapshot;
```

In `src/main.rs`, add to the module list (after `mod cache;`, keeping alpha-ish order is fine):

```rust
mod cluster;
```

- [ ] **Step 2: Write the failing round-trip test**

Create `src/cluster/snapshot.rs` with the struct, a stub, and the test:

```rust
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::auth::Auth;
use crate::config::categories::get_categories as get_cf_categories;
use crate::config::styles::get_styles;
use crate::error::AppResult;
use crate::models::{catalog::Catalog, category::Category, styles::Style};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub catalog: Catalog,
    pub categories: Vec<Category>,
    pub auth: Auth,
    pub styles: Vec<Style>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn snapshot_round_trips_through_json() {
        let pool = in_memory_pool().await;
        let snap = build_snapshot("config", &pool).await.unwrap();
        let json = serde_json::to_string(&snap).unwrap();
        let back: ConfigSnapshot = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test --lib cluster::snapshot`
Expected: FAIL — `cannot find function build_snapshot`.

- [ ] **Step 4: Implement `build_snapshot` and `apply_snapshot`**

In `src/cluster/snapshot.rs`, add (above the `#[cfg(test)]` module):

```rust
/// Builds a full config snapshot from the SQLite config DB.
pub async fn build_snapshot(config_dir: &str, pool: &SqlitePool) -> AppResult<ConfigSnapshot> {
    Ok(ConfigSnapshot {
        catalog: Catalog::new(pool).await?,
        categories: get_cf_categories(Some(pool)).await?,
        auth: Auth::new(config_dir, pool).await?,
        styles: get_styles(Some(pool)).await?,
    })
}

/// Swaps the four in-memory states under their RwLocks. `config_dir` is the
/// local instance's value and overrides whatever the snapshot's Auth carried,
/// so a client does not inherit the owner's paths.
pub async fn apply_snapshot(snapshot: ConfigSnapshot, config_dir: &str) {
    let ConfigSnapshot { catalog, categories, mut auth, styles } = snapshot;
    auth.config_dir = config_dir.to_string();

    *crate::get_catalog().await.write().await = catalog;
    *crate::get_categories().await.write().await = categories;
    *crate::get_auth().await.write().await = auth;
    *crate::get_styles_cache().await.write().await = styles;
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --lib cluster::snapshot`
Expected: PASS.

(Note: `apply_snapshot` will warn as unused until Task 9; that is a warning, not an error.)

- [ ] **Step 6: Commit**

```bash
git add src/cluster/mod.rs src/cluster/snapshot.rs src/main.rs
git commit -m "feat(cluster): add ConfigSnapshot with build/apply"
```

---

### Task 9: Sync backend + watcher; wire into `shared` mode

**Files:**
- Create: `src/cluster/backend.rs`
- Create: `src/cluster/watcher.rs`
- Modify: `src/cluster/mod.rs` (register both)
- Modify: `src/main.rs` (spawn the watcher for `shared` mode)

**Interfaces:**
- Consumes: `get_config_version` (Task 1), `build_snapshot`/`apply_snapshot` (Task 8), `get_cf_pool`.
- Produces:
  - `cluster::backend::SyncBackend` enum with `current_version(&self) -> AppResult<i64>` and `fetch_snapshot(&self, config_dir: &str) -> AppResult<ConfigSnapshot>`. (Remote arm is added in Task 11.)
  - `cluster::watcher::next_known_version(known: i64, current: i64, reload_ok: bool) -> i64`
  - `cluster::watcher::start_config_watcher(backend: SyncBackend, interval: Duration, config_dir: String)`

- [ ] **Step 1: Write the failing test for `next_known_version`**

Create `src/cluster/watcher.rs`:

```rust
/// Decides the next "known" version after a poll tick. The known version only
/// advances when a higher version was observed AND the reload succeeded, so a
/// failed reload is retried on the next tick.
pub fn next_known_version(known: i64, current: i64, reload_ok: bool) -> i64 {
    if current > known && reload_ok { current } else { known }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_when_version_unchanged() {
        assert_eq!(next_known_version(5, 5, true), 5);
    }

    #[test]
    fn advances_when_higher_and_reload_ok() {
        assert_eq!(next_known_version(5, 6, true), 6);
    }

    #[test]
    fn stays_when_reload_failed() {
        assert_eq!(next_known_version(5, 6, false), 5);
    }
}
```

- [ ] **Step 2: Register the modules**

In `src/cluster/mod.rs`:

```rust
pub mod backend;
pub mod snapshot;
pub mod watcher;
```

- [ ] **Step 3: Run the test to verify it passes (and compiles)**

Run: `cargo test --lib cluster::watcher`
Expected: the 3 unit tests PASS. (`backend` module is empty/missing — add it in Step 4 first if compilation fails on the `mod backend;` line; create an empty `src/cluster/backend.rs` to satisfy it, then proceed.)

- [ ] **Step 4: Implement the `SyncBackend` enum (Local arm only)**

Create/replace `src/cluster/backend.rs`:

```rust
use sqlx::SqlitePool;

use crate::cluster::snapshot::{ConfigSnapshot, build_snapshot};
use crate::config::system_settings::get_config_version;
use crate::error::AppResult;

/// Where an instance reads the config version and snapshot from.
pub enum SyncBackend {
    /// Reads from the local SQLite pool (shared-volume / owner).
    Local { pool: &'static SqlitePool },
    // Remote arm added in Task 11.
}

impl SyncBackend {
    pub async fn current_version(&self) -> AppResult<i64> {
        match self {
            SyncBackend::Local { pool } => Ok(get_config_version(pool).await?),
        }
    }

    pub async fn fetch_snapshot(&self, config_dir: &str) -> AppResult<ConfigSnapshot> {
        match self {
            SyncBackend::Local { pool } => build_snapshot(config_dir, pool).await,
        }
    }
}
```

- [ ] **Step 5: Implement the watcher loop**

Add to `src/cluster/watcher.rs` (above the `#[cfg(test)]` module):

```rust
use std::time::Duration;
use tracing::{info, warn};

use crate::cluster::backend::SyncBackend;
use crate::cluster::snapshot::apply_snapshot;

/// Spawns a background task that polls the config version every `interval` and,
/// when it rises, fetches a snapshot and swaps the in-memory state. A failed
/// reload is retried on the next tick (known version is not advanced).
pub fn start_config_watcher(backend: SyncBackend, interval: Duration, config_dir: String) {
    tokio::spawn(async move {
        let mut known = backend.current_version().await.unwrap_or(0);

        loop {
            tokio::time::sleep(interval).await;

            let current = match backend.current_version().await {
                Ok(v) => v,
                Err(e) => {
                    warn!("config watcher: failed to poll version: {e}");
                    continue;
                }
            };

            if current > known {
                let reload_ok = match backend.fetch_snapshot(&config_dir).await {
                    Ok(snapshot) => {
                        apply_snapshot(snapshot, &config_dir).await;
                        info!("config watcher: reloaded in-memory state ({known} -> {current})");
                        true
                    }
                    Err(e) => {
                        warn!("config watcher: reload failed, will retry next tick: {e}");
                        false
                    }
                };
                known = next_known_version(known, current, reload_ok);
            }
        }
    });
}
```

- [ ] **Step 6: Spawn the watcher for `shared` mode in `main.rs`**

In `src/main.rs`, add near the other `std` imports:

```rust
use std::time::Duration;
```

After the globals are set (after `STYLES.set(...)` from Task 6) and before `let i18n_service = ...`, add:

```rust
    if settings.cluster.mode == "shared" {
        cluster::watcher::start_config_watcher(
            cluster::backend::SyncBackend::Local { pool: get_cf_pool() },
            Duration::from_secs(settings.cluster.config_watch_interval_secs),
            settings.paths.config.clone(),
        );
    }
```

- [ ] **Step 7: Build and run tests**

Run: `cargo build`
Expected: compiles (Remote-related code does not exist yet; no reference to it).

Run: `cargo test --lib cluster`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/cluster/backend.rs src/cluster/watcher.rs src/cluster/mod.rs src/main.rs
git commit -m "feat(cluster): add sync backend + watcher; enable shared mode"
```

---

### Task 10: Internal API on the owner + wire into the router

**Files:**
- Create: `src/cluster/api.rs`
- Modify: `src/cluster/mod.rs` (register `api`)
- Modify: `src/main.rs` (new `CLUSTER_SECRET` and `CONFIG_DIR` globals, set them)
- Modify: `src/routes.rs` (mount `/internal` when `mode = owner`)

**Interfaces:**
- Consumes: `build_snapshot` (Task 8), `get_config_version` (Task 1), `get_cf_pool`, new `get_cluster_secret`/`get_config_dir`.
- Produces:
  - `crate::get_cluster_secret() -> &'static str`, `crate::get_config_dir() -> &'static str`
  - `cluster::api::build_internal_routes() -> salvo::Router`
  - Internal routes: `GET /internal/config/version` → `{"version":N}`, `GET /internal/config/snapshot` → `ConfigSnapshot` JSON, both behind an `X-Cluster-Secret` guard.

- [ ] **Step 1: Write the failing test for the constant-time compare**

Create `src/cluster/api.rs`:

```rust
/// Constant-time byte comparison (avoids leaking secret length-prefix matches
/// via timing). Returns true only when both slices are equal.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_matches_only_equal_slices() {
        assert!(ct_eq(b"secret", b"secret"));
        assert!(!ct_eq(b"secret", b"secres"));
        assert!(!ct_eq(b"secret", b"secre"));
    }
}
```

- [ ] **Step 2: Register the module and run the test**

In `src/cluster/mod.rs` add `pub mod api;`.

Run: `cargo test --lib cluster::api`
Expected: PASS (1 test).

- [ ] **Step 3: Add the `CLUSTER_SECRET` and `CONFIG_DIR` globals in `main.rs`**

In `src/main.rs`, after the `JWT_SECRET` global block, add:

```rust
static CLUSTER_SECRET: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_cluster_secret() -> &'static str {
    CLUSTER_SECRET.get().map(|s| s.as_str()).unwrap_or("")
}

static CONFIG_DIR: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_config_dir() -> &'static str {
    CONFIG_DIR.get().map(|s| s.as_str()).unwrap_or("")
}
```

Where the other globals are set (after `JWT_SECRET.set(...)`), add:

```rust
    CONFIG_DIR.set(settings.paths.config.clone()).unwrap();
    if let Some(secret) = settings.cluster.shared_secret.clone() {
        CLUSTER_SECRET.set(secret).unwrap();
    }
```

- [ ] **Step 4: Implement the guard, handlers, and router builder**

Add to `src/cluster/api.rs` (above the `#[cfg(test)]` module):

```rust
use salvo::prelude::*;
use serde_json::json;

use crate::cluster::snapshot::build_snapshot;
use crate::config::system_settings::get_config_version;
use crate::error::{AppError, AppResult};
use crate::{get_cf_pool, get_cluster_secret, get_config_dir};

/// Rejects requests whose `X-Cluster-Secret` header does not match the
/// configured cluster secret. The internal API ships config (incl. password
/// hashes), so it must never be reachable without the secret.
#[handler]
async fn cluster_secret_guard(req: &mut Request, res: &mut Response, ctrl: &mut FlowCtrl) {
    let provided = req
        .headers()
        .get("x-cluster-secret")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let expected = get_cluster_secret();
    if expected.is_empty() || !ct_eq(provided.as_bytes(), expected.as_bytes()) {
        res.status_code(StatusCode::UNAUTHORIZED);
        res.render(Json(json!({ "error": "unauthorized" })));
        ctrl.skip_rest();
    }
}

#[handler]
async fn version(res: &mut Response) -> AppResult<()> {
    let v = get_config_version(get_cf_pool())
        .await
        .map_err(AppError::from)?;
    res.render(Json(json!({ "version": v })));
    Ok(())
}

#[handler]
async fn snapshot(res: &mut Response) -> AppResult<()> {
    let snap = build_snapshot(get_config_dir(), get_cf_pool()).await?;
    res.render(Json(snap));
    Ok(())
}

/// `/internal/config/{version,snapshot}` behind the cluster-secret guard.
pub fn build_internal_routes() -> Router {
    Router::with_path("internal/config")
        .hoop(cluster_secret_guard)
        .push(Router::with_path("version").get(version))
        .push(Router::with_path("snapshot").get(snapshot))
}
```

- [ ] **Step 5: Mount `/internal` for owner mode in `routes.rs`**

In `src/routes.rs`, inside `app_router`, change the router construction so the internal routes are added only for `owner`. Replace the `let router = Router::new()...;` chain's end by binding it mutably and pushing conditionally:

```rust
    let mut router = Router::new()
        .options(handler::empty())
        .hoop(Logger::default())
        .hoop(affix_state::inject(i18n_service))
        .hoop(session_handler)
        .push(build_public_routes())
        .push(build_api_routes())
        .push(Router::with_path("health").get(health::get_health))
        .push(build_services_routes(settings, cache_5s))
        .push(Router::with_path("static/{**path}").get(serve_static));

    if settings.cluster.mode == "owner" {
        router = router.push(crate::cluster::api::build_internal_routes());
    }
```

- [ ] **Step 6: Write a failing integration test for the guard**

Create `tests/integration/cluster_internal.rs` (and ensure it is included by the integration harness — see the existing files under `tests/integration/` and mirror how they are declared in `tests/`). Use salvo's test client against the internal router with the secret set via the global.

```rust
use mvt_rs::cluster::api::build_internal_routes;
use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};

// NOTE: build_internal_routes() reads crate::get_cluster_secret(). In tests the
// global is unset, so get_cluster_secret() returns "" and the guard must reject
// everything. This verifies the fail-closed behavior.
#[tokio::test]
async fn snapshot_without_secret_is_unauthorized() {
    let service = Service::new(build_internal_routes());
    let status = TestClient::get("http://127.0.0.1:5800/internal/config/snapshot")
        .send(&service)
        .await
        .status_code
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
```

If `mvt_rs` is a binary-only crate (no `lib.rs`), expose the needed items by adding a minimal `src/lib.rs` that `pub use`s them, or place this test as a `#[cfg(test)]` unit test inside `src/cluster/api.rs` using `Service::new(build_internal_routes())` directly. Prefer the in-module unit test if there is no `lib.rs`:

```rust
    #[tokio::test]
    async fn snapshot_without_secret_is_unauthorized() {
        use salvo::prelude::*;
        use salvo::test::TestClient;
        let service = Service::new(build_internal_routes());
        let resp = TestClient::get("http://127.0.0.1:5800/internal/config/snapshot")
            .send(&service)
            .await;
        assert_eq!(resp.status_code.unwrap(), StatusCode::UNAUTHORIZED);
    }
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test --lib cluster::api`
Expected: PASS (the guard rejects with 401 when no secret is configured/sent).

- [ ] **Step 8: Build the whole project**

Run: `cargo build`
Expected: compiles.

- [ ] **Step 9: Commit**

```bash
git add src/cluster/api.rs src/cluster/mod.rs src/main.rs src/routes.rs
git commit -m "feat(cluster): internal config API (version/snapshot) behind secret guard; owner mode"
```

---

### Task 11: `reqwest` dependency + `RemoteBackend`

**Files:**
- Modify: `Cargo.toml` (add `reqwest`)
- Modify: `src/error.rs` (add `reqwest::Error` variant)
- Modify: `src/cluster/backend.rs` (add the `Remote` arm + constructor)

**Interfaces:**
- Consumes: the internal API (Task 10), `ConfigSnapshot` (Task 8).
- Produces: `SyncBackend::Remote { owner_url: String, secret: String, client: reqwest::Client }`, reachable via `SyncBackend::remote(owner_url, secret)`.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]`, add:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

- [ ] **Step 2: Add the error variant**

In `src/error.rs`, inside `pub enum AppError`, add a variant (mirroring the other `#[from]` variants; include the matching `#[error(...)]` attribute used by the enum):

```rust
    #[error("HTTP client error: {0}")]
    HttpClientError(#[from] reqwest::Error),
```

- [ ] **Step 3: Write the failing integration test (Remote against a live owner router)**

Add to `src/cluster/backend.rs` a `#[cfg(test)]` module that serves the internal router with a known secret and points a `Remote` backend at it.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn remote_reads_version_from_owner_api() {
        // This test documents the Remote contract. It requires the owner's
        // internal router and a reachable HTTP endpoint; it is wired in Step 5.
        let backend = SyncBackend::remote(
            "http://127.0.0.1:5899".to_string(),
            "test-secret".to_string(),
        );
        // With no server running, current_version must return an Err (not panic).
        assert!(backend.current_version().await.is_err());
    }
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test --lib cluster::backend`
Expected: FAIL — `no function remote` / missing `Remote` arm.

- [ ] **Step 5: Implement the `Remote` arm**

In `src/cluster/backend.rs`, extend the enum and impl:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct VersionResponse {
    version: i64,
}

pub enum SyncBackend {
    Local { pool: &'static SqlitePool },
    Remote {
        owner_url: String,
        secret: String,
        client: reqwest::Client,
    },
}

impl SyncBackend {
    pub fn remote(owner_url: String, secret: String) -> Self {
        SyncBackend::Remote {
            owner_url: owner_url.trim_end_matches('/').to_string(),
            secret,
            client: reqwest::Client::new(),
        }
    }

    pub async fn current_version(&self) -> AppResult<i64> {
        match self {
            SyncBackend::Local { pool } => Ok(get_config_version(pool).await?),
            SyncBackend::Remote { owner_url, secret, client } => {
                let resp: VersionResponse = client
                    .get(format!("{owner_url}/internal/config/version"))
                    .header("x-cluster-secret", secret)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                Ok(resp.version)
            }
        }
    }

    pub async fn fetch_snapshot(&self, config_dir: &str) -> AppResult<ConfigSnapshot> {
        match self {
            SyncBackend::Local { pool } => build_snapshot(config_dir, pool).await,
            SyncBackend::Remote { owner_url, secret, client } => {
                let snapshot: ConfigSnapshot = client
                    .get(format!("{owner_url}/internal/config/snapshot"))
                    .header("x-cluster-secret", secret)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                Ok(snapshot)
            }
        }
    }
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test --lib cluster::backend`
Expected: PASS (no server at :5899 ⇒ `current_version` returns `Err`).

- [ ] **Step 7: Build the project**

Run: `cargo build`
Expected: compiles.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/error.rs src/cluster/backend.rs
git commit -m "feat(cluster): add reqwest RemoteBackend for cross-host config sync"
```

---

### Task 12: Client mode — bootstrap, startup branch, reduced router

**Files:**
- Create: `src/cluster/bootstrap.rs`
- Modify: `src/cluster/mod.rs` (register `bootstrap`)
- Modify: `src/main.rs` (branch startup on `client` mode)
- Modify: `src/routes.rs` (client reduced router; `app_router` dispatches by mode)

**Interfaces:**
- Consumes: `SyncBackend::remote` (Task 11), `apply_snapshot` (Task 8), `start_config_watcher` (Task 9).
- Produces:
  - `cluster::bootstrap::bootstrap_from_owner(owner_url: &str, secret: &str, interval: Duration) -> ConfigSnapshot` (retries until the owner responds)
  - `routes::client_app_router(settings, i18n) -> Service`

- [ ] **Step 1: Implement `bootstrap_from_owner`**

Create `src/cluster/bootstrap.rs`:

```rust
use std::time::Duration;
use tracing::{info, warn};

use crate::cluster::backend::SyncBackend;
use crate::cluster::snapshot::ConfigSnapshot;

/// Fetches the initial config snapshot from the owner, retrying every
/// `retry_interval` until it succeeds. A client cannot serve correct config
/// without it, so this intentionally blocks startup until the owner is reachable.
pub async fn bootstrap_from_owner(
    owner_url: &str,
    secret: &str,
    retry_interval: Duration,
) -> ConfigSnapshot {
    let backend = SyncBackend::remote(owner_url.to_string(), secret.to_string());
    loop {
        match backend.fetch_snapshot("").await {
            Ok(snapshot) => {
                info!("cluster client: fetched initial config snapshot from owner");
                return snapshot;
            }
            Err(e) => {
                warn!("cluster client: owner not ready ({e}); retrying in {retry_interval:?}");
                tokio::time::sleep(retry_interval).await;
            }
        }
    }
}
```

In `src/cluster/mod.rs` add `pub mod bootstrap;`.

- [ ] **Step 2: Add the client reduced router in `routes.rs`**

In `src/routes.rs`, add a new function (mirrors `app_router` but mounts only reads):

```rust
/// Reduced router for `cluster.mode = client`: no SQLite, so only memory-served
/// reads (tiles/styles/legends), health, and static assets are mounted. Admin,
/// write API, and `/internal` are intentionally absent (nginx routes those to
/// the owner).
pub fn client_app_router(settings: &Settings, i18n_service: Arc<I18n>) -> Service {
    let cache_5s = build_cache_middleware(5);
    let cors_handler = build_cors_handler();
    let session_handler = build_session_handler(settings);

    let router = Router::new()
        .options(handler::empty())
        .hoop(Logger::default())
        .hoop(affix_state::inject(i18n_service))
        .hoop(session_handler)
        .push(Router::with_path("health").get(health::get_health))
        .push(build_services_routes(settings, cache_5s))
        .push(Router::with_path("static/{**path}").get(serve_static));

    Service::new(router)
        .hoop(strip_tile_cookie)
        .hoop(cors_handler)
        .catcher(Catcher::default().hoop(html::errors::handle_errors))
}
```

- [ ] **Step 3: Dispatch by mode in `app_router`**

At the top of `app_router` in `src/routes.rs`, add:

```rust
    if settings.cluster.mode == "client" {
        return client_app_router(settings, i18n_service);
    }
```

- [ ] **Step 4: Branch startup in `main.rs`**

In `src/main.rs`, replace the linear init block (from `let cf_pool = config::db::init_sqlite(...)` through the `STYLES.set(...)` line) with a branch. The non-client path is the current code plus the styles load from Task 6; the client path skips SQLite and fills the globals from the snapshot.

```rust
    CONFIG_DIR.set(settings.paths.config.clone()).unwrap();

    if settings.cluster.mode == "client" {
        let owner_url = settings.cluster.owner_url.clone().expect("client requires owner_url");
        let secret = settings.cluster.shared_secret.clone().expect("client requires shared_secret");
        CLUSTER_SECRET.set(secret.clone()).unwrap();

        let snapshot = cluster::bootstrap::bootstrap_from_owner(
            &owner_url,
            &secret,
            Duration::from_secs(settings.cluster.config_watch_interval_secs),
        )
        .await;

        // Initialize globals that do not depend on the config SQLite.
        let db_registry = DbRegistry::new(
            &settings.postgres_databases.connections,
            settings.postgres_databases.pool_min,
            settings.postgres_databases.pool_max,
        )
        .await?;
        let cache_wrapper = CacheWrapper::initialize_cache(
            settings.database.redis_url.clone(),
            settings.paths.cache.clone().into(),
            snapshot.catalog.clone(),
        )
        .await?;
        let plugin_registry = plugins::LuaPluginRegistry::new(&settings.paths.plugins);

        DB_REGISTRY.set(db_registry).unwrap();
        MAP_ASSETS_DIR.set(settings.paths.assets.clone()).unwrap();
        JWT_SECRET.set(settings.security.jwt_secret.clone()).unwrap();
        CACHE_WRAPPER.set(cache_wrapper).unwrap();
        PLUGIN_REGISTRY.set(plugin_registry).unwrap();

        // Initialize the four in-memory states from the snapshot.
        CATALOG.set(RwLock::new(snapshot.catalog.clone())).unwrap();
        CATEGORIES.set(RwLock::new(snapshot.categories.clone())).unwrap();
        STYLES.set(RwLock::new(snapshot.styles.clone())).unwrap();
        {
            let mut auth = snapshot.auth.clone();
            auth.config_dir = settings.paths.config.clone();
            AUTH.set(RwLock::new(auth)).unwrap();
        }

        cluster::watcher::start_config_watcher(
            cluster::backend::SyncBackend::remote(owner_url, secret),
            Duration::from_secs(settings.cluster.config_watch_interval_secs),
            settings.paths.config.clone(),
        );
    } else {
        // ... existing standalone/shared/owner path:
        //   init_sqlite, initialize_auth/catalog, get_cf_categories, get_styles,
        //   DbRegistry, CacheWrapper, plugin_registry,
        //   set DB_REGISTRY/SQLITE_CONF/MAP_ASSETS_DIR/JWT_SECRET/CACHE_WRAPPER/
        //   PLUGIN_REGISTRY/CATALOG/CATEGORIES/AUTH/STYLES,
        //   set CLUSTER_SECRET (Task 10), and spawn the shared-mode watcher (Task 9).
    }
```

Keep the existing `else` body exactly as it already is after Tasks 6/9/10 (do not duplicate the lines here — move the already-written init into the `else` block). Ensure `SQLITE_CONF` is set **only** in the `else` branch (a client has no config pool).

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: compiles. Watch for: any handler mounted in `client_app_router` that calls `get_cf_pool()` — there must be none (tiles/styles/legends now read memory; styles/legends were moved in Task 6). If the compiler/usage shows otherwise, fix the read path to use the cache before proceeding.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 7: Manual cross-instance smoke test**

1. Start an owner with a temp config dir and a short interval:
   ```bash
   MVT_CLUSTER__MODE=owner \
   MVT_CLUSTER__SHARED_SECRET=test-secret \
   MVT_CLUSTER__CONFIG_WATCH_INTERVAL_SECS=3 \
   MVT_SERVER__PORT=5887 cargo run
   ```
2. Start a client pointing at it:
   ```bash
   MVT_CLUSTER__MODE=client \
   MVT_CLUSTER__OWNER_URL=http://127.0.0.1:5887 \
   MVT_CLUSTER__SHARED_SECRET=test-secret \
   MVT_CLUSTER__CONFIG_WATCH_INTERVAL_SECS=3 \
   MVT_SERVER__PORT=5888 cargo run
   ```
3. On the owner admin panel (`:5887`) create/rename a layer and edit a style.
4. Within ~3s the client (`:5888`) logs `config watcher: reloaded in-memory state`.
5. Confirm without restarting the client:
   - `GET :5888/api/catalog/layer` reflects the layer change.
   - `GET :5888/services/styles/{cat:name}` returns the updated style.
   - A protected tile request to `:5888` with `Authorization: Basic ...` validates (synced hashes).
   - `GET :5887/internal/config/snapshot` (no secret) → `401`.

- [ ] **Step 8: Commit**

```bash
git add src/cluster/bootstrap.rs src/cluster/mod.rs src/main.rs src/routes.rs
git commit -m "feat(cluster): client mode — bootstrap from owner, reduced router, remote watcher"
```

---

### Task 13: Documentation & examples

**Files:**
- Modify: `config.example.yaml` (add the `cluster:` block)
- Create: `docs/clustering.md`
- Modify: `README.md` (link the new doc under "Advanced usage")

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Add the `cluster:` block to `config.example.yaml`**

Append to `config.example.yaml`:

```yaml
# ─── Clustering / multi-instance ──────────────────────────────────────────────
# Keep in-memory config (catalog, categories, users, groups, styles) fresh across
# several instances behind a load balancer. Default is a single standalone server.
cluster:
  # standalone | shared | owner | client
  #   standalone : single instance (default).
  #   shared     : several instances on the SAME host sharing one SQLite file
  #                via a shared volume (situation 1).
  #   owner      : holds the single SQLite and exposes the internal sync API
  #                (situation 2, cross-host).
  #   client     : no local SQLite; pulls config from the owner over HTTP
  #                (situation 2, cross-host).
  mode: "standalone"

  # How often (seconds) each non-owner instance polls for config changes.
  config_watch_interval_secs: 10

  # Required when mode = client: base URL of the owner instance.
  # owner_url: "https://owner-host:5887"

  # Required when mode = owner or client: shared secret authorizing the internal
  # /internal/config/* API. Use a strong random value (>= 16 chars). That API
  # ships config including password hashes, so this traffic MUST go over TLS or a
  # trusted private network, and /internal must not be exposed publicly.
  # shared_secret: "change-me-to-a-random-cluster-secret"

# Env-var equivalents: MVT_CLUSTER__MODE, MVT_CLUSTER__CONFIG_WATCH_INTERVAL_SECS,
# MVT_CLUSTER__OWNER_URL, MVT_CLUSTER__SHARED_SECRET
```

- [ ] **Step 2: Write `docs/clustering.md`**

Create `docs/clustering.md` with these sections (prose + the config/nginx snippets):

1. **Overview** — the staleness problem (each instance caches config in memory; a write on one instance leaves others stale) and the single-SQLite constraint.
2. **Situation 1 — same host (shared volume):** set `mode: shared` on every instance, point them all at the same SQLite file via a shared volume, tune `config_watch_interval_secs`.
3. **Situation 2 — different hosts (owner/client):** roles, `owner_url`, `shared_secret`, the internal API; GeoServer master/slave analogy.
4. **Load balancer (nginx):**

   ```nginx
   upstream mvt_owner { server owner-host:5887; }
   upstream mvt_tiles { server owner-host:5887;
                        server client1-host:5887;
                        server client2-host:5887; }

   # admin panel + login + config API => the owner (the only writer)
   location /admin     { proxy_pass http://mvt_owner; }
   location /auth      { proxy_pass http://mvt_owner; }
   location /api/admin { proxy_pass http://mvt_owner; }

   # reads (tiles, styles, legends are all under /services) => balanced pool
   location /services/ { proxy_pass http://mvt_tiles; }
   location /          { proxy_pass http://mvt_tiles; }

   # /internal must NOT be exposed publicly (no location routes it)
   ```

5. **Security:** cluster secret; TLS or private network is mandatory cross-host (the snapshot carries password hashes); never expose `/internal`.
6. **Behavior & limits:** propagation delay ≤ interval; owner is the single writer; a client needs the owner reachable to cold-start (it retries until then); tile-cache invalidation across instances is out of scope.
7. **Verification:** the two-instance smoke test from Task 12 Step 7.

- [ ] **Step 3: Link the doc from the README**

In `README.md`, under the "Advanced usage" area (near the existing "Advanced usage (Styles, Legends, Sprites, Glyphs)" line), add:

```markdown
- [Clustering / multi-instance behind a load balancer](docs/clustering.md)
```

- [ ] **Step 4: Commit**

```bash
git add config.example.yaml docs/clustering.md README.md
git commit -m "docs(cluster): document clustering config, nginx, and security"
```

---

## Self-Review Notes

- **Spec coverage:** modes/roles (Tasks 7, 9, 10, 12); `config_version` + bumps incl. styles (Tasks 1–5); styles cached uniformly + memory read path + inline refresh (Task 6); `ConfigSnapshot` (Task 8); backend abstraction + watcher (Tasks 9, 11); internal API + secret guard (Task 10); client bootstrap + reduced router (Task 12); backward-compatible default verified by `standalone` having no watcher/no internal API/full router (Tasks 9/10/12 gate on mode) and the Task 12 manual step; reqwest dependency (Task 11); docs + examples (Task 13). Security (hashes kept, TLS/private network, no public `/internal`) is enforced by the guard (Task 10) and documented (Task 13).
- **Type consistency:** `get_config_version`/`bump_config_version` return `Result<_, sqlx::Error>`; `build_snapshot`/`fetch_snapshot`/`apply_snapshot` use `AppResult`/`ConfigSnapshot`; `SyncBackend::{current_version, fetch_snapshot, remote}` names match across Tasks 9/11/12; `start_config_watcher(SyncBackend, Duration, String)` matches both call sites (shared in Task 9, client in Task 12); `get_styles_cache`/`reload_styles_cache`/`find_style`/`from_category_and_name_cached` names match across Tasks 6/8/10.
- **Known limitations (out of scope):** cross-host HA of the owner (single writer; clients keep serving from memory if it is down but config cannot be edited and new clients cannot cold-start); tile-cache invalidation across instances; a local snapshot disk cache for client cold-start while the owner is down.
- **Verification caveats:** Task 10 Step 6 depends on whether the crate exposes a `lib.rs`; the plan provides an in-module unit-test fallback. Task 12's full client path is validated by the manual smoke test (touches global singletons and the network), backed by the automated `RemoteBackend` test (Task 11) and the guard test (Task 10).
```
