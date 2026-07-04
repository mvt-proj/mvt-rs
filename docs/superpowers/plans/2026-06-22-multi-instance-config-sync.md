# Multi-Instance Config Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the in-memory config state (`Catalog`, `Categories`, `Auth`) fresh across multiple server instances that share one SQLite config DB, so an admin edit on one instance propagates to all of them.

**Architecture:** A single global monotonic counter `config_version`, stored in the existing `system_settings` table, is bumped inside every SQLite config-write function. A background watcher in each instance polls that counter every N seconds; when it increases, the watcher rebuilds `Catalog`, `Categories` and `Auth` from the shared SQLite DB and swaps them under their `RwLock`s. No new runtime dependencies. The shared SQLite file lives on a volume shared between same-host containers/jails (WAL works because they share a kernel).

**Tech Stack:** Rust, SQLx 0.8 (SQLite), Tokio, Salvo. Migrations via `sqlx::migrate!`.

## Global Constraints

- No new external service dependency (no Postgres/Redis requirement for config). Config store stays SQLite.
- Config-write functions in `src/config/*.rs` return `Result<_, sqlx::Error>`. The version helpers used by them MUST also return `Result<_, sqlx::Error>` so `?` composes without conversion.
- The reload path (`Catalog::new`, `Auth::new`, etc.) returns `AppResult<T>`; `AppError` already implements `From<sqlx::Error>`.
- Single editor in practice, negligible write concurrency — no write-coordination/locking logic is needed; a plain counter is sufficient.
- Styles are NOT cached in memory (no global), so they are intentionally excluded: no bump and no reload for styles.
- Leave the orphan `src/plugins/watcher.rs` untouched (out of scope).
- Existing pattern to mirror (but do NOT reuse the orphan code): `get_plugins_version`/`bump_plugins_version` in `src/config/system_settings.rs`.

---

### Task 1: `config_version` infrastructure + shared test pool

**Files:**
- Create: `migrations/20260622120000_config_version.sql`
- Modify: `src/config/system_settings.rs` (add two functions + tests)
- Create: `src/config/test_support.rs` (shared in-memory pool helper for tests)
- Modify: `src/config/mod.rs:1-8` (register `test_support` under `cfg(test)`)

**Interfaces:**
- Produces:
  - `pub async fn get_config_version(pool: &sqlx::SqlitePool) -> Result<i64, sqlx::Error>`
  - `pub async fn bump_config_version(pool: &sqlx::SqlitePool) -> Result<i64, sqlx::Error>` (returns the new value)
  - `crate::config::test_support::in_memory_pool() -> sqlx::SqlitePool` (cfg(test) only; runs all migrations)

- [ ] **Step 1: Add the migration row**

Create `migrations/20260622120000_config_version.sql`:

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

/// In-memory SQLite pool with all migrations applied. `max_connections(1)`
/// is required: each connection to `sqlite::memory:` gets its own database,
/// so a multi-connection pool would run migrations on one DB and queries on
/// another.
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

- [ ] **Step 4: Write the failing tests for the version helpers**

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

- [ ] **Step 6: Implement the version helpers**

Append to `src/config/system_settings.rs` (before the `#[cfg(test)]` module), mirroring the existing `plugins_version` helpers but returning `sqlx::Error`:

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

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --lib config::system_settings`
Expected: PASS (2 tests).

- [ ] **Step 8: Commit**

```bash
git add migrations/20260622120000_config_version.sql src/config/system_settings.rs src/config/test_support.rs src/config/mod.rs
git commit -m "feat(config): add config_version counter and shared test pool"
```

---

### Task 2: Bump `config_version` on category writes

**Files:**
- Modify: `src/config/categories.rs` (3 write functions + tests)

**Interfaces:**
- Consumes: `bump_config_version` from Task 1.
- Produces: `create_category`/`update_category`/`delete_category` now bump the version after a successful write (signatures unchanged).

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

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::categories`
Expected: FAIL — `get_config_version() == 0`, asserts expect 1/2/1.

- [ ] **Step 3: Add the import**

At the top of `src/config/categories.rs`, after `use crate::get_cf_pool;`:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to `create_category`**

In `create_category`, between the `INSERT ... .execute(pool).await?;` block and `Ok(())`:

```rust
    sqlx::query("INSERT INTO categories (id, name, description) VALUES (?, ?, ?)")
        .bind(&category.id)
        .bind(&category.name)
        .bind(&category.description)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

- [ ] **Step 5: Add the bump to `update_category`**

In `update_category`, after the `UPDATE categories ... .execute(pool).await?;` block and before `Ok(())`:

```rust
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

- [ ] **Step 6: Add the bump to `delete_category`**

In `delete_category`, after the `DELETE FROM categories ... .execute(pool).await?;` block and before `Ok(())`:

```rust
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --lib config::categories`
Expected: PASS (3 tests).

- [ ] **Step 8: Commit**

```bash
git add src/config/categories.rs
git commit -m "feat(config): bump config_version on category writes"
```

---

### Task 3: Bump `config_version` on layer writes

**Files:**
- Modify: `src/config/layers.rs` (4 write functions: `create_layer`, `update_layer`, `delete_layer`, `switch_layer_published` + test)

**Interfaces:**
- Consumes: `bump_config_version` from Task 1.
- Produces: all four layer write functions bump the version after a successful write (signatures unchanged).

- [ ] **Step 1: Write the failing test**

`delete_layer` only needs an id (no row required), so it is the clean test seam for this file. Append to `src/config/layers.rs`:

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
Expected: FAIL — version is 0, expected 1.

- [ ] **Step 3: Add the import**

At the top of `src/config/layers.rs`, after `use crate::get_cf_pool;`:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to all four write functions**

In each of `create_layer`, `update_layer`, `delete_layer`, and `switch_layer_published`, insert `bump_config_version(pool).await?;` immediately after the final `.execute(pool).await?;` and before `Ok(())`. For `switch_layer_published` it goes after the `UPDATE` execute (not the `SELECT`). Example for `delete_layer`:

```rust
    sqlx::query("DELETE FROM layers WHERE id = ?")
        .bind(layer_id)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

And `switch_layer_published`:

```rust
    sqlx::query("UPDATE layers SET published = ? WHERE id = ?")
        .bind(!published)
        .bind(layer_id)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

Apply the same one-line insertion to `create_layer` and `update_layer` after their respective `.execute(pool).await?;`.

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
- Consumes: `bump_config_version` from Task 1.
- Produces: all user/group write functions bump the version after a successful write (signatures unchanged).

- [ ] **Step 1: Write the failing tests**

`delete_user`/`delete_group` only need an id, so they are the clean test seams.

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

At the top of both `src/config/users.rs` and `src/config/groups.rs`, after the existing `use crate::get_cf_pool;` line:

```rust
use crate::config::system_settings::bump_config_version;
```

- [ ] **Step 4: Add the bump to all six write functions**

In each of `create_user`, `update_user`, `delete_user` (users.rs) and `create_group`, `update_group`, `delete_group` (groups.rs), insert `bump_config_version(pool).await?;` immediately after the `.execute(pool).await?;` and before `Ok(())`. Example for `delete_group`:

```rust
    sqlx::query("DELETE FROM Groups WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib config::users config::groups`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add src/config/users.rs src/config/groups.rs
git commit -m "feat(config): bump config_version on user and group writes"
```

---

### Task 5: Config watcher module

**Files:**
- Create: `src/config/watcher.rs`
- Modify: `src/config/mod.rs` (add `pub mod watcher;`)

**Interfaces:**
- Consumes: `get_config_version` (Task 1); `Catalog::new`, `Auth::new`, `config::categories::get_categories`, and the globals `get_cf_pool`/`get_catalog`/`get_categories`/`get_auth`.
- Produces: `pub fn start_config_watcher(config_dir: String, interval: std::time::Duration)` — spawns the polling task. (Used by Task 6.)

- [ ] **Step 1: Write the failing test for the advance/retry decision**

The subtle behavior is: only advance the known version when the reload actually succeeded, so a failed reload retries on the next tick. Create `src/config/watcher.rs` with just the helper + test:

```rust
/// Decides the next "known" version after a poll tick. The known version only
/// advances when a higher version was observed AND the reload succeeded, so a
/// failed reload is retried on the next tick.
fn next_known_version(known: i64, current: i64, reload_ok: bool) -> i64 {
    if current > known && reload_ok {
        current
    } else {
        known
    }
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

- [ ] **Step 2: Register the module and run the test to verify it fails to compile/pass**

In `src/config/mod.rs`, add after the existing `pub mod` lines:

```rust
pub mod watcher;
```

Run: `cargo test --lib config::watcher`
Expected: At this point the 3 unit tests should PASS (the helper is pure). This step's purpose is to confirm the module compiles and is wired. If `cargo` reports `function is never used` as a hard error, proceed to Step 3 which adds the caller.

- [ ] **Step 3: Implement the watcher**

Replace the contents of `src/config/watcher.rs` with the full module (keep the test block from Step 1 at the bottom):

```rust
use std::time::Duration;
use tracing::{info, warn};

use crate::auth::Auth;
use crate::config::categories::get_categories as get_cf_categories;
use crate::config::system_settings::get_config_version;
use crate::error::AppResult;
use crate::models::catalog::Catalog;
use crate::{get_auth, get_catalog, get_categories, get_cf_pool};

/// Spawns a background task that polls `system_settings.config_version` every
/// `interval`. When the version in the shared SQLite DB is higher than the
/// locally known one, the instance rebuilds Catalog, Categories and Auth from
/// the DB and swaps them under their RwLocks. This is how an admin edit on one
/// instance propagates to the others behind the load balancer.
pub fn start_config_watcher(config_dir: String, interval: Duration) {
    tokio::spawn(async move {
        let pool = get_cf_pool();
        let mut known = get_config_version(pool).await.unwrap_or(0);

        loop {
            tokio::time::sleep(interval).await;

            let current = match get_config_version(pool).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("config watcher: failed to poll version: {e}");
                    continue;
                }
            };

            if current > known {
                let reload_ok = match reload_all_state(&config_dir, pool).await {
                    Ok(()) => {
                        info!("config watcher: reloaded in-memory state ({known} → {current})");
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

/// Rebuilds the three in-memory config states from the shared SQLite DB and
/// swaps them in place under their RwLocks.
async fn reload_all_state(config_dir: &str, pool: &sqlx::SqlitePool) -> AppResult<()> {
    let catalog = Catalog::new(pool).await?;
    let categories = get_cf_categories(Some(pool)).await?;
    let auth = Auth::new(config_dir, pool).await?;

    *get_catalog().await.write().await = catalog;
    *get_categories().await.write().await = categories;
    *get_auth().await.write().await = auth;

    Ok(())
}

/// Decides the next "known" version after a poll tick. The known version only
/// advances when a higher version was observed AND the reload succeeded, so a
/// failed reload is retried on the next tick.
fn next_known_version(known: i64, current: i64, reload_ok: bool) -> i64 {
    if current > known && reload_ok {
        current
    } else {
        known
    }
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

- [ ] **Step 4: Run tests and a type check**

Run: `cargo test --lib config::watcher`
Expected: PASS (3 tests).

Run: `cargo check`
Expected: compiles (note: `start_config_watcher`/`reload_all_state` will warn as unused until Task 6 wires them in — a warning, not an error).

- [ ] **Step 5: Commit**

```bash
git add src/config/watcher.rs src/config/mod.rs
git commit -m "feat(config): add config watcher that reloads in-memory state on version bump"
```

---

### Task 6: Configurable interval + wire the watcher into startup

**Files:**
- Modify: `src/config/settings.rs` (add `config_watch_interval_secs` to `ServerConfig`, default fn, `set_default`, and fix the test constructor)
- Modify: `src/main.rs` (import `Duration`, spawn the watcher after globals are set)

**Interfaces:**
- Consumes: `start_config_watcher` (Task 5); `Settings.server.config_watch_interval_secs`.
- Produces: each running instance spawns the watcher at startup.

- [ ] **Step 1: Write the failing test for the default**

Add to the `tests` module in `src/config/settings.rs`:

```rust
    #[test]
    fn config_watch_interval_default_is_ten() {
        assert_eq!(default_config_watch_interval(), 10);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib config::settings`
Expected: FAIL — `cannot find function default_config_watch_interval`.

- [ ] **Step 3: Add the field, default, and builder default**

In `src/config/settings.rs`, extend `ServerConfig`:

```rust
#[derive(Debug, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_config_watch_interval")]
    pub config_watch_interval_secs: u64,
}
```

Add the default function next to `default_port`:

```rust
fn default_config_watch_interval() -> u64 { 10 }
```

Add a `set_default` in the builder chain in `Settings::new`, right after the `server.port` default:

```rust
            .set_default("server.config_watch_interval_secs", 10)?
```

- [ ] **Step 4: Fix the existing test constructor**

In the `tests` module, the `valid_settings()` helper builds `ServerConfig` with a struct literal — add the new field so it still compiles:

```rust
            server: ServerConfig { host: "0.0.0.0".to_string(), port: 5887, config_watch_interval_secs: 10 },
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib config::settings`
Expected: PASS (existing tests + `config_watch_interval_default_is_ten`).

- [ ] **Step 6: Wire the watcher into `main.rs`**

In `src/main.rs`, add the import near the other `std` imports (line ~5):

```rust
use std::time::Duration;
```

Then, after the global initialization block (immediately after `AUTH.set(RwLock::new(auth)).unwrap();`, around line 198) and before `let i18n_service = ...`, add:

```rust
    config::watcher::start_config_watcher(
        settings.paths.config.clone(),
        Duration::from_secs(settings.server.config_watch_interval_secs),
    );
```

- [ ] **Step 7: Verify the build and full test suite**

Run: `cargo build`
Expected: compiles with no `unused` warnings for `start_config_watcher`/`reload_all_state`.

Run: `cargo test`
Expected: all tests PASS.

- [ ] **Step 8: Commit**

```bash
git add src/config/settings.rs src/main.rs
git commit -m "feat(config): spawn config watcher at startup with configurable interval"
```

---

### Task 7: Manual cross-instance verification + docs

**Files:**
- Modify: `CLAUDE.md` (document the new optional setting under "Optional security settings" → add a "Clustering / multi-instance" note)
- Modify: `.env.example` (add `MVT_SERVER__CONFIG_WATCH_INTERVAL_SECS`)

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Manual two-instance smoke test**

This behavior touches global singletons and a shared file, so it is verified manually (the automated tests cover the version mechanism and the advance/retry decision).

1. Point two instances at the **same** SQLite file (shared volume / same `paths.config`), with a short interval for the test:
   ```bash
   MVT_SERVER__CONFIG_WATCH_INTERVAL_SECS=3 MVT_SERVER__PORT=5887 cargo run &
   MVT_SERVER__CONFIG_WATCH_INTERVAL_SECS=3 MVT_SERVER__PORT=5888 cargo run &
   ```
2. In the admin panel of instance A (`:5887`), create or rename a layer/category.
3. Within ~3s, instance B (`:5888`) logs `config watcher: reloaded in-memory state (N → N+1)`.
4. Confirm the change is visible on B (e.g. `GET :5888/api/catalog/layer` or the admin catalog page) **without restarting B**.

Expected: B reflects A's change within one interval.

- [ ] **Step 2: Document the setting**

In `CLAUDE.md`, under the configuration section, add:

```markdown
**Clustering / multi-instance:**
- `server.config_watch_interval_secs` — how often (seconds) each instance polls the shared SQLite `config_version` counter to refresh its in-memory Catalog/Categories/Auth (default: `10`). Env var: `MVT_SERVER__CONFIG_WATCH_INTERVAL_SECS`. Requires all instances to share the same SQLite config file (e.g. a shared volume across same-host containers/jails).
```

In `.env.example`, add:

```bash
# How often (seconds) each instance polls the shared SQLite config_version
# counter to refresh in-memory config across a multi-instance deployment.
MVT_SERVER__CONFIG_WATCH_INTERVAL_SECS=10
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md .env.example
git commit -m "docs(config): document config_watch_interval_secs and multi-instance sync"
```

---

## Self-Review Notes

- **Spec coverage:** Problem 1 (shared persistence) is a deployment choice (shared volume) — no code. Problem 2 (in-memory staleness) is covered by Tasks 1–6: bump on every cached-config write (categories, layers, users, groups) + watcher reload of the three cached states (Catalog, Categories, Auth). Styles are correctly excluded (not cached in memory).
- **Type consistency:** `get_config_version`/`bump_config_version` return `Result<_, sqlx::Error>` to compose with the `config/*.rs` write functions; `reload_all_state` returns `AppResult<()>` and relies on `From<sqlx::Error> for AppError`. `start_config_watcher(config_dir: String, interval: Duration)` matches the call site in `main.rs`.
- **Known limitation (out of scope):** the shared-volume approach binds all instances to one host; cross-host HA would need a different store (Litestream or a shared DB). The tile cache is also out of scope per the requirements (a layer deleted on A stays cached in Redis/disk until separately invalidated).
