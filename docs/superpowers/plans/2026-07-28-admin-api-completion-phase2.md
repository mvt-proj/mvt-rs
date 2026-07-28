# Admin API Completion — Phase 2 (Groups/Categories/Styles CRUD) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add JSON REST CRUD for Groups, Categories, and Styles to `src/api/`
(currently entirely missing), plus a shared delete-time "in use" guard for
Group/Category and two `Style` parity fixes that Phase 1 already gave
`Category`.

**Architecture:** Three new files (`src/api/groups.rs`,
`src/api/categories.rs`, `src/api/styles.rs`), each with a `list`/`create`/
`update`/`delete` handler set and a `build_api_<entity>_routes()` factory
mirroring the existing `build_api_users_routes()` /
`build_api_database_routes()` / `build_api_catalog_routes()` pattern in
`src/routes.rs`. Small, targeted edits to existing files:
`src/error.rs` (new `Conflict` variant), `src/auth/models.rs` (pure
reference-counting helper + `Group::delete_group` guard),
`src/models/category.rs` (pure reference-counting helper +
`Category::delete_category` guard), `src/models/styles.rs` (404 mapping +
internal cache sync), `src/html/admin/styles.rs` (drop now-redundant manual
cache-reload calls), `src/api/mod.rs`, `src/routes.rs`.

**Tech Stack:** Rust, Salvo 0.95 (`Extractible` derive with per-field
`source(from = "param")` override for path params in a body-sourced
struct), SQLx, serde/serde_json.

## Global Constraints

- All of `src/api/` returns `AppResult<T>` / `AppError` — no manual status
  codes, no `Result<T, StatusError>`.
- Update endpoints are full replacement (all editable fields required in
  the body) — no partial/`PATCH` semantics.
- `Style.category` is referenced by id (via `Category::from_id`), not by
  name — matches the existing HTML style form and Phase 1's `create_layer`.
- The delete-time "in use" guard lives once in the model layer
  (`Group::delete_group`, `Category::delete_category`), not duplicated per
  HTML/API handler — both surfaces inherit it automatically.
- The guard's reference-counting logic is a **pure function** (no global
  state access) taking borrowed slices, following the existing
  `find_style` precedent in `src/models/styles.rs:117-119` — this is what
  makes it genuinely unit-testable, unlike the methods that call it.
- The guard scans existing in-memory global caches
  (`get_catalog().read().await.layers`, `get_auth().read().await.users`,
  the `STYLES` cache) — no new SQL queries.
- Only touch files this plan lists. In particular, do not touch
  `src/main.rs` or refactor the auth gap on `GET /api/catalog/layer` — both
  are explicitly out of scope for this phase (see the design spec's "Out of
  scope" section). `src/main.rs` currently has an unrelated, already-tested,
  uncommitted change on the working tree (a `CryptoProvider::install_default`
  fix) — do not touch this file at all.
- Full design context: `docs/superpowers/specs/2026-07-28-admin-api-completion-phase2-design.md`.

---

### Task 1: Add `AppError::Conflict` variant

**Files:**
- Modify: `src/error.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `AppError::Conflict(String)` → `StatusCode::CONFLICT` (409),
  used by Task 4 (`Group::delete_group`) and Task 5
  (`Category::delete_category`).

- [ ] **Step 1: Add the variant**

In `src/error.rs`, find:

```rust
    #[error("Forbidden: {0}")]
    Forbidden(String),
```

Add right after it:

```rust
    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),
```

- [ ] **Step 2: Map it to 409 in `status_code`**

Find:

```rust
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
```

Add right after it:

```rust
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
```

- [ ] **Step 3: Build to confirm it compiles**

Run: `cargo build`
Expected: builds clean (new enum variant, exhaustively-matched `status_code`
already has a `_ =>` fallback arm so this addition is additive-safe either
way, but the explicit arm is added for clarity, matching `Forbidden`'s
treatment).

- [ ] **Step 4: Commit**

```bash
git add src/error.rs
git commit -m "feat(error): add AppError::Conflict (409) variant"
```

---

### Task 2: Fix `Style::from_id` to map "not found" to 404

**Files:**
- Modify: `src/models/styles.rs:57-61` (`Style::from_id`)
- Test: `src/models/styles.rs` (extend existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `config::styles::get_style` (existing, unchanged signature:
  `get_style(id: &str, pool: Option<&SqlitePool>) -> Result<Style, sqlx::Error>`).
- Produces: `Style::from_id` now returns `AppError::NotFound` for a missing
  id instead of leaking `AppError::SQLError` (500) — mirrors
  `Category::from_id` from Phase 1. Used by Task 8's update/delete API
  handlers.

- [ ] **Step 1: Write a regression test for the precondition this relies on**

Same rationale as Phase 1 Task 4: `Style::from_id` itself can't be
called in isolation (it goes through `get_style(id, None)`, and `None`
means "use the global config pool" — a `OnceLock` uninitialized in unit
tests). This test locks down the `sqlx::Error::RowNotFound` behavior the
next step's `match` arm depends on, using the pool-injectable `get_style`
directly.

In `src/models/styles.rs`, add to the existing `#[cfg(test)] mod tests`
block (after the existing `find_style_matches_category_name_and_style_name`
test):

```rust
    #[tokio::test]
    async fn get_style_returns_row_not_found_for_unknown_id() {
        use crate::config::test_support::in_memory_pool;
        let pool = in_memory_pool().await;
        let result = crate::config::styles::get_style("missing-id", Some(&pool)).await;
        assert!(matches!(result, Err(sqlx::Error::RowNotFound)));
    }
```

- [ ] **Step 2: Run it to confirm it passes**

Run: `cargo test get_style_returns_row_not_found_for_unknown_id -- --nocapture`
Expected: PASS (regression guard, not a failing-first test — see Step 1's
note).

- [ ] **Step 3: Map `RowNotFound` to `AppError::NotFound` in `Style::from_id`**

Replace:

```rust
    pub async fn from_id(id: &str) -> AppResult<Self> {
        let style = get_style(id, None).await?;

        Ok(style)
    }
```

with:

```rust
    pub async fn from_id(id: &str) -> AppResult<Self> {
        match get_style(id, None).await {
            Ok(style) => Ok(style),
            Err(sqlx::Error::RowNotFound) => {
                Err(AppError::NotFound(format!("Style {id} not found")))
            }
            Err(e) => Err(e.into()),
        }
    }
```

`AppError` is already imported at the top of this file
(`error::{AppError, AppResult}`) — no import change needed.

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. (Existing callers —
`html/admin/styles.rs::edit_style_page/update_style/delete_style` — already
propagate the error generically via `?`/match on `AppResult`, so a
different `AppError` variant doesn't break them.)

- [ ] **Step 5: Commit**

```bash
git add src/models/styles.rs
git commit -m "fix(styles): map Style::from_id RowNotFound to AppError::NotFound"
```

---

### Task 3: Sync the `STYLES` cache inside `Style`'s model methods

**Files:**
- Modify: `src/models/styles.rs` (`Style::new`, `Style::update_style`,
  `Style::delete_style`)
- Modify: `src/html/admin/styles.rs` (`create_style`, `update_style`,
  `delete_style` — drop the now-redundant manual cache-reload calls)

**Interfaces:**
- Consumes: `crate::reload_styles_cache()` (existing, in `src/main.rs` —
  not modified by this task).
- Produces: `Style::new`/`update_style`/`delete_style` now keep the global
  `STYLES` cache in sync themselves. Any caller (HTML today, the new API
  handlers from Task 8) gets this for free without remembering to call
  `reload_styles_cache()` afterward.

- [ ] **Step 1: Add the cache-sync call to all three mutating methods**

In `src/models/styles.rs`, replace:

```rust
        create_style(style.clone(), None).await?;

        Ok(style)
    }
```

(inside `Style::new`) with:

```rust
        create_style(style.clone(), None).await?;
        crate::reload_styles_cache().await?;

        Ok(style)
    }
```

Replace:

```rust
        update_style(style.clone(), None).await?;

        Ok(style)
    }
```

(inside `Style::update_style`) with:

```rust
        update_style(style.clone(), None).await?;
        crate::reload_styles_cache().await?;

        Ok(style)
    }
```

Replace:

```rust
    pub async fn delete_style(&self) -> AppResult<()> {
        delete_style(&self.id, None).await?;
        Ok(())
    }
```

with:

```rust
    pub async fn delete_style(&self) -> AppResult<()> {
        delete_style(&self.id, None).await?;
        crate::reload_styles_cache().await?;
        Ok(())
    }
```

Note: `create_style`/`update_style`/`delete_style` (no `crate::` prefix) are
the imported `config::styles::*` free functions; `crate::reload_styles_cache`
is the crate-root function in `src/main.rs` — the two are distinguishable by
the `crate::` prefix and don't collide with the identically-named module
functions.

- [ ] **Step 2: Build to confirm it compiles**

Run: `cargo build`
Expected: builds clean.

- [ ] **Step 3: Remove the now-redundant manual calls in the HTML handlers**

In `src/html/admin/styles.rs`, there are three identical lines to remove
(each currently sits right before a `res.headers_mut()...Redirect::other("/admin/styles")` block):

In `create_style` (around line 125):

```rust
    crate::reload_styles_cache().await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}
```

becomes:

```rust
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}
```

Apply the exact same removal (delete the `crate::reload_styles_cache().await?;`
line, keep everything else) in `update_style` (around line 172) and
`delete_style` (around line 200). There are three occurrences total in this
file — remove all three.

- [ ] **Step 4: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. This is a pure behavior-preserving
move (the cache reload still happens, just inside the model instead of the
caller) — `crate::get_styles_cache()` is a process-global `OnceCell` not
swappable in unit tests, so, consistent with the rest of this codebase's
cache-touching code, there's no new isolated unit test for this; correctness
rests on this being a mechanical move plus the build/regression check.

- [ ] **Step 5: Commit**

```bash
git add src/models/styles.rs src/html/admin/styles.rs
git commit -m "fix(styles): sync STYLES cache inside Style model methods instead of per-caller"
```

---

### Task 4: Add delete-time "in use" guard for `Group`

**Files:**
- Modify: `src/auth/models.rs` (new pure function + `Group::delete_group`)
- Test: `src/auth/tests.rs`

**Interfaces:**
- Consumes: `User.groups: Vec<Group>`, `Layer.groups: Option<Vec<Group>>`
  (existing fields), `AppError::Conflict` (Task 1).
- Produces: `count_group_references(group_id: &str, users: &[User], layers: &[Layer]) -> (usize, usize)`
  (pure, returns `(user_count, layer_count)`) and an updated
  `Group::delete_group` that returns `AppError::Conflict` instead of
  deleting when either count is nonzero. Both the existing HTML
  `html/admin/groups.rs::delete_group` handler and Task 6's new API delete
  handler call `Group::delete_group` unchanged and inherit this guard
  automatically.

- [ ] **Step 1: Write the failing tests for the pure counting function**

Add to `src/auth/tests.rs`, inside the existing `mod tests { ... }` block.
This needs a `Layer` value (no existing builder in this file — add a small
local one, mirroring the existing `test_layer()` helper pattern in
`src/services/tilejson.rs:280-315`) and `User` values, for which this file
already has a `create_test_user(auth: &Auth, username, email, password,
groups: Vec<Group>) -> User` helper (line 26) — reuse it instead of adding
a second one:

```rust
    use crate::models::catalog::Layer;
    use crate::models::category::Category;

    fn test_layer(id: &str, groups: Option<Vec<Group>>) -> Layer {
        Layer {
            id: id.to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "public".to_string(),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "layer".to_string(),
            alias: "Layer".to_string(),
            description: String::new(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "t".to_string(),
            fields: vec![],
            filter: None,
            srid: None,
            geom: None,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age: None,
            max_records: None,
            published: true,
            url: None,
            groups,
        }
    }

    #[test]
    fn test_count_group_references_none() {
        let auth = create_test_auth();
        let other_group = Group {
            id: "other".to_string(),
            name: "other".to_string(),
            description: String::new(),
        };
        let users = vec![create_test_user(
            &auth,
            "u1",
            "u1@test.com",
            "pw",
            vec![other_group.clone()],
        )];
        let layers = vec![test_layer("l1", Some(vec![other_group]))];
        assert_eq!(count_group_references("target", &users, &layers), (0, 0));
    }

    #[test]
    fn test_count_group_references_counts_users_and_layers() {
        let auth = create_test_auth();
        let target_group = Group {
            id: "target".to_string(),
            name: "target".to_string(),
            description: String::new(),
        };
        let other_group = Group {
            id: "other".to_string(),
            name: "other".to_string(),
            description: String::new(),
        };
        let users = vec![
            create_test_user(&auth, "u1", "u1@test.com", "pw", vec![target_group.clone()]),
            create_test_user(
                &auth,
                "u2",
                "u2@test.com",
                "pw",
                vec![target_group.clone(), other_group.clone()],
            ),
            create_test_user(&auth, "u3", "u3@test.com", "pw", vec![other_group]),
        ];
        let layers = vec![
            test_layer("l1", Some(vec![target_group])),
            test_layer("l2", None),
        ];
        assert_eq!(count_group_references("target", &users, &layers), (2, 1));
    }

    #[test]
    fn test_count_group_references_empty_input() {
        assert_eq!(count_group_references("target", &[], &[]), (0, 0));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test count_group_references -- --nocapture`
Expected: FAIL to compile — `count_group_references` doesn't exist yet.

- [ ] **Step 3: Implement the pure counting function**

In `src/auth/models.rs`, add the import for `Layer` at the top. Find:

```rust
use crate::config::groups::{create_group, delete_group, get_groups, update_group};
use crate::config::users::{create_user, delete_user, get_users, update_user};
use crate::error::{AppError, AppResult};
use crate::{get_auth, get_jwt_secret};
```

Replace with:

```rust
use crate::config::groups::{create_group, delete_group, get_groups, update_group};
use crate::config::users::{create_user, delete_user, get_users, update_user};
use crate::error::{AppError, AppResult};
use crate::models::catalog::Layer;
use crate::{get_auth, get_catalog, get_jwt_secret};
```

Then add the pure function right after the `Group` struct's closing
`impl Group { ... }` block (after `delete_group`'s existing closing `}` at
what is currently line 115), before the `User` struct:

```rust
/// Counts how many users and layers still reference `group_id`. Pure so it
/// can be unit-tested without the global `Auth`/`Catalog` state that
/// `Group::delete_group` reads it from.
pub fn count_group_references(group_id: &str, users: &[User], layers: &[Layer]) -> (usize, usize) {
    let user_count = users
        .iter()
        .filter(|user| user.groups.iter().any(|group| group.id == group_id))
        .count();
    let layer_count = layers
        .iter()
        .filter(|layer| {
            layer
                .groups
                .as_ref()
                .is_some_and(|groups| groups.iter().any(|group| group.id == group_id))
        })
        .count();
    (user_count, layer_count)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test count_group_references -- --nocapture`
Expected: PASS

- [ ] **Step 5: Wire the guard into `Group::delete_group`**

Replace:

```rust
    pub async fn delete_group(&self) -> AppResult<()> {
        let mut auth = get_auth().await.write().await;
        let position = auth.groups.iter().position(|group| group.id == self.id);

        delete_group(self.id.clone(), None).await?;

        match position {
            Some(pos) => {
                auth.groups.remove(pos);
            }
            None => {
                warn!(group_id = %self.id, "Group not found during deletion");
            }
        }

        Ok(())
    }
```

with:

```rust
    pub async fn delete_group(&self) -> AppResult<()> {
        let catalog = get_catalog().await.read().await;
        let layers = catalog.layers.clone();
        drop(catalog);

        let mut auth = get_auth().await.write().await;
        let (user_count, layer_count) = count_group_references(&self.id, &auth.users, &layers);

        if user_count > 0 || layer_count > 0 {
            return Err(AppError::Conflict(format!(
                "Group '{}' is in use by {user_count} user(s) and {layer_count} layer(s)",
                self.name
            )));
        }

        let position = auth.groups.iter().position(|group| group.id == self.id);

        delete_group(self.id.clone(), None).await?;

        match position {
            Some(pos) => {
                auth.groups.remove(pos);
            }
            None => {
                warn!(group_id = %self.id, "Group not found during deletion");
            }
        }

        Ok(())
    }
```

(The `catalog` read lock is acquired and dropped *before* the `auth` write
lock is taken, matching the lock-ordering convention already used elsewhere
in this codebase, e.g. `html/admin/catalog.rs:119-120` — never holding both
locks at once avoids introducing any new deadlock risk.)

- [ ] **Step 6: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. (`Group::delete_group` itself
touches `get_auth()`/`get_catalog()` global state, so — consistent with
every other model method in this codebase — it isn't unit-testable in
isolation; its correctness rests on the already-tested
`count_group_references` plus this build/regression check. This also means
`html/admin/groups.rs::delete_group`, unchanged, now blocks in-use deletes
too — a behavior improvement, not a regression.)

- [ ] **Step 7: Commit**

```bash
git add src/auth/models.rs src/auth/tests.rs
git commit -m "feat(auth): block Group::delete_group when the group is still referenced"
```

---

### Task 5: Add delete-time "in use" guard for `Category`

**Files:**
- Modify: `src/models/category.rs` (new pure function + `Category::delete_category`)
- Test: `src/models/category.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Layer.category: Category`, `Style.category: Category`
  (existing fields), `AppError::Conflict` (Task 1).
- Produces: `count_category_references(category_id: &str, layers: &[Layer], styles: &[Style]) -> (usize, usize)`
  (pure) and an updated `Category::delete_category` that returns
  `AppError::Conflict` instead of deleting when either count is nonzero.
  Both `html/admin/categories.rs::delete_category` (unchanged) and Task 7's
  new API delete handler inherit this automatically.

- [ ] **Step 1: Write the failing tests for the pure counting function**

Add to `src/models/category.rs`'s existing `#[cfg(test)] mod tests` block
(after the existing `get_category_by_id_returns_row_not_found_for_unknown_id`
test). This needs `Layer` and `Style` values — add a `use super::*;` (the
module doesn't have one yet, only `use crate::config::test_support::in_memory_pool;`)
plus the two type imports:

```rust
    use super::*;
    use crate::models::catalog::Layer;
    use crate::models::styles::Style;

    fn test_layer_with_category(id: &str, category_id: &str) -> Layer {
        Layer {
            id: id.to_string(),
            category: Category {
                id: category_id.to_string(),
                name: format!("cat-{category_id}"),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "layer".to_string(),
            alias: "Layer".to_string(),
            description: String::new(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "t".to_string(),
            fields: vec![],
            filter: None,
            srid: None,
            geom: None,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
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

    fn test_style_with_category(id: &str, category_id: &str) -> Style {
        Style {
            id: id.to_string(),
            name: "style".to_string(),
            category: Category {
                id: category_id.to_string(),
                name: format!("cat-{category_id}"),
                description: String::new(),
            },
            description: String::new(),
            style: "{}".to_string(),
        }
    }

    #[test]
    fn test_count_category_references_none() {
        let layers = vec![test_layer_with_category("l1", "other")];
        let styles = vec![test_style_with_category("s1", "other")];
        assert_eq!(count_category_references("target", &layers, &styles), (0, 0));
    }

    #[test]
    fn test_count_category_references_counts_layers_and_styles() {
        let layers = vec![
            test_layer_with_category("l1", "target"),
            test_layer_with_category("l2", "target"),
            test_layer_with_category("l3", "other"),
        ];
        let styles = vec![test_style_with_category("s1", "target")];
        assert_eq!(count_category_references("target", &layers, &styles), (2, 1));
    }

    #[test]
    fn test_count_category_references_empty_input() {
        assert_eq!(count_category_references("target", &[], &[]), (0, 0));
    }
```

Note this adds a second `use super::*;` if one already exists at the top of
the test module — check first: the existing test module currently only has
`use crate::config::test_support::in_memory_pool;`, no `use super::*;`, so
add it once.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test count_category_references -- --nocapture`
Expected: FAIL to compile — `count_category_references` doesn't exist yet.

- [ ] **Step 3: Implement the pure counting function**

In `src/models/category.rs`, find the top imports:

```rust
use serde::{Deserialize, Serialize};

use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::{AppError, AppResult},
    get_catalog, get_categories,
};
```

Replace with:

```rust
use serde::{Deserialize, Serialize};

use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::{AppError, AppResult},
    get_catalog, get_categories, get_styles_cache,
    models::{catalog::Layer, styles::Style},
};
```

Then add the pure function right after the `Category` struct definition,
before `impl Category`:

```rust
/// Counts how many layers and styles still reference `category_id`. Pure so
/// it can be unit-tested without the global `Catalog`/`STYLES` state that
/// `Category::delete_category` reads it from.
pub fn count_category_references(category_id: &str, layers: &[Layer], styles: &[Style]) -> (usize, usize) {
    let layer_count = layers.iter().filter(|l| l.category.id == category_id).count();
    let style_count = styles.iter().filter(|s| s.category.id == category_id).count();
    (layer_count, style_count)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test count_category_references -- --nocapture`
Expected: PASS

- [ ] **Step 5: Wire the guard into `Category::delete_category`**

Replace:

```rust
    pub async fn delete_category(&self) -> AppResult<()> {
        delete_category(None, &self.id.clone()).await?;
        let mut categories = get_categories().await.write().await;

        let position = categories.iter().position(|c| c.id == self.id);

        if let Some(pos) = position {
            categories.remove(pos);
        }

        Ok(())
    }
```

with:

```rust
    pub async fn delete_category(&self) -> AppResult<()> {
        let catalog = get_catalog().await.read().await;
        let layers = catalog.layers.clone();
        drop(catalog);

        let styles_cache = get_styles_cache().await.read().await;
        let styles = styles_cache.clone();
        drop(styles_cache);

        let (layer_count, style_count) = count_category_references(&self.id, &layers, &styles);

        if layer_count > 0 || style_count > 0 {
            return Err(AppError::Conflict(format!(
                "Category '{}' is in use by {layer_count} layer(s) and {style_count} style(s)",
                self.name
            )));
        }

        delete_category(None, &self.id.clone()).await?;
        let mut categories = get_categories().await.write().await;

        let position = categories.iter().position(|c| c.id == self.id);

        if let Some(pos) = position {
            categories.remove(pos);
        }

        Ok(())
    }
```

- [ ] **Step 6: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. Same rationale as Task 4 Step 6:
`Category::delete_category` itself isn't unit-testable in isolation (global
state), correctness rests on `count_category_references` plus this
build/regression check; `html/admin/categories.rs::delete_category`,
unchanged, inherits the guard.

- [ ] **Step 7: Commit**

```bash
git add src/models/category.rs
git commit -m "feat(category): block Category::delete_category when the category is still referenced"
```

---

### Task 6: Groups JSON API

**Files:**
- Create: `src/api/groups.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/routes.rs`

**Interfaces:**
- Consumes: `Group::new`, `Group::from_id`, `group.update_group`,
  `group.delete_group` (existing, `delete_group` now guarded per Task 4).
- Produces: `GET/POST /api/admin/groups`, `PUT/DELETE /api/admin/groups/{id}`.

- [ ] **Step 1: Create `src/api/groups.rs`**

```rust
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth,
};

#[handler]
pub async fn list(res: &mut Response) {
    let auth = get_auth().await.read().await;
    res.render(Json(&auth.groups));
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewGroupRequest {
    name: String,
    description: String,
}

#[handler]
pub async fn create(res: &mut Response, data: NewGroupRequest) -> AppResult<()> {
    let group = Group::new(data.name, data.description).await?;
    res.render(Json(&group));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateGroupRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
    name: String,
    description: String,
}

#[handler]
pub async fn update(res: &mut Response, data: UpdateGroupRequest) -> AppResult<()> {
    let group = Group::from_id(&data.id).await?;
    let updated = group.update_group(data.name, data.description).await?;
    res.render(Json(&updated));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let group = Group::from_id(&id).await?;
    group.delete_group().await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
```

- [ ] **Step 2: Register the module**

In `src/api/mod.rs`, replace:

```rust
pub mod catalog;
pub mod database;
pub mod users;
```

with:

```rust
pub mod catalog;
pub mod database;
pub mod groups;
pub mod users;
```

- [ ] **Step 3: Wire the routes**

In `src/routes.rs`, add a new router-factory function right after
`build_api_users_routes()` (currently lines 218-222):

```rust
fn build_api_groups_routes() -> Router {
    Router::with_path("groups")
        .get(api::groups::list)
        .post(api::groups::create)
        .push(
            Router::with_path("{id}")
                .put(api::groups::update)
                .delete(api::groups::delete),
        )
}
```

Then find `build_api_routes()`'s `admin` block:

```rust
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .hoop(auth::validate_token)
                .hoop(auth::require_api_admin)
                .push(build_api_users_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
        )
```

Add `build_api_groups_routes()` to the pushed list:

```rust
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .hoop(auth::validate_token)
                .hoop(auth::require_api_admin)
                .push(build_api_users_routes())
                .push(build_api_groups_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
        )
```

- [ ] **Step 4: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. These handlers touch `get_auth()`
global state (same limitation as every other `src/api/` handler in this
codebase — see Phase 1 Task 3's note) — no isolated handler-level unit
test; correctness rests on the already-tested `Group` model methods (Task 4)
plus this build/regression check.

- [ ] **Step 5: Manual smoke check**

With the dev server running (`cargo watch -x run` or `cargo run`), obtain an
admin JWT via `POST /api/users/login`, then exercise the new endpoints with
`curl`:

```bash
TOKEN="<jwt from login>"
curl -s -X POST http://localhost:5887/api/admin/groups \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"name": "smoke-test-group", "description": "temp"}'
curl -s http://localhost:5887/api/admin/groups -H "Authorization: Bearer $TOKEN"
```

Confirm: create returns the new group with a generated `id`, list includes
it, `PUT /api/admin/groups/{id}` updates it, and
`DELETE /api/admin/groups/{id}` removes it (clean up the smoke-test group
this way rather than leaving it in the DB).

- [ ] **Step 6: Commit**

```bash
git add src/api/groups.rs src/api/mod.rs src/routes.rs
git commit -m "feat(api): add Groups CRUD to the JSON admin API"
```

---

### Task 7: Categories JSON API

**Files:**
- Create: `src/api/categories.rs`
- Modify: `src/routes.rs`

**Interfaces:**
- Consumes: `Category::new`, `Category::from_id`, `category.update_category`,
  `category.delete_category` (existing, `delete_category` now guarded per
  Task 5).
- Produces: `GET/POST /api/admin/categories`,
  `PUT/DELETE /api/admin/categories/{id}`.

- [ ] **Step 1: Create `src/api/categories.rs`**

```rust
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    get_categories,
    models::category::Category,
};

#[handler]
pub async fn list(res: &mut Response) {
    let categories = get_categories().await.read().await;
    res.render(Json(categories.to_vec()));
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewCategoryRequest {
    name: String,
    description: String,
}

#[handler]
pub async fn create(res: &mut Response, data: NewCategoryRequest) -> AppResult<()> {
    let category = Category::new(data.name, data.description).await?;
    res.render(Json(&category));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateCategoryRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
    name: String,
    description: String,
}

#[handler]
pub async fn update(res: &mut Response, data: UpdateCategoryRequest) -> AppResult<()> {
    let category = Category::from_id(&data.id).await?;
    let updated = category.update_category(data.name, data.description).await?;
    res.render(Json(&updated));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let category = Category::from_id(&id).await?;
    category.delete_category().await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
```

- [ ] **Step 2: Register the module**

In `src/api/mod.rs`, replace:

```rust
pub mod catalog;
pub mod database;
pub mod groups;
pub mod users;
```

with:

```rust
pub mod catalog;
pub mod categories;
pub mod database;
pub mod groups;
pub mod users;
```

- [ ] **Step 3: Wire the routes**

In `src/routes.rs`, add a new router-factory function right after
`build_api_groups_routes()` (added in Task 6):

```rust
fn build_api_categories_routes() -> Router {
    Router::with_path("categories")
        .get(api::categories::list)
        .post(api::categories::create)
        .push(
            Router::with_path("{id}")
                .put(api::categories::update)
                .delete(api::categories::delete),
        )
}
```

In `build_api_routes()`'s `admin` block, add it to the pushed list:

```rust
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .hoop(auth::validate_token)
                .hoop(auth::require_api_admin)
                .push(build_api_users_routes())
                .push(build_api_groups_routes())
                .push(build_api_categories_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
        )
```

- [ ] **Step 4: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. Same rationale as Task 6 Step 4 —
no isolated handler-level unit test; correctness rests on the
already-tested `Category` model methods (Task 5) plus this build/regression
check.

- [ ] **Step 5: Manual smoke check**

Same pattern as Task 6 Step 5, against `/api/admin/categories`. Additionally
verify the guard end-to-end: create a category, create a layer or style
referencing it (or reuse an existing one from your local data), attempt
`DELETE /api/admin/categories/{id}`, confirm it returns `409` with the
expected message, then delete the referencing layer/style first and confirm
the category delete then succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/api/categories.rs src/api/mod.rs src/routes.rs
git commit -m "feat(api): add Categories CRUD to the JSON admin API"
```

---

### Task 8: Styles JSON API

**Files:**
- Create: `src/api/styles.rs`
- Modify: `src/routes.rs`

**Interfaces:**
- Consumes: `Style::get_all_styles`, `Style::new`, `Style::from_id`
  (now 404-safe per Task 2), `style.update_style`, `style.delete_style`
  (now cache-syncing per Task 3), `Category::from_id`,
  `services::utils::validate_style_json` (all existing).
- Produces: `GET/POST /api/admin/styles`, `PUT/DELETE /api/admin/styles/{id}`.

- [ ] **Step 1: Create `src/api/styles.rs`**

```rust
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    models::{category::Category, styles::Style},
};

#[handler]
pub async fn list(res: &mut Response) -> AppResult<()> {
    let styles = Style::get_all_styles().await?;
    res.render(Json(&styles));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewStyleRequest {
    name: String,
    category: String,
    description: String,
    style: String,
}

#[handler]
pub async fn create(res: &mut Response, data: NewStyleRequest) -> AppResult<()> {
    crate::services::utils::validate_style_json(&data.style)?;
    let category = Category::from_id(&data.category).await?;
    let style = Style::new(data.name, category, data.description, data.style).await?;
    res.render(Json(&style));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateStyleRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
    name: String,
    category: String,
    description: String,
    style: String,
}

#[handler]
pub async fn update(res: &mut Response, data: UpdateStyleRequest) -> AppResult<()> {
    let style = Style::from_id(&data.id).await?;
    crate::services::utils::validate_style_json(&data.style)?;
    let category = Category::from_id(&data.category).await?;
    let updated = style
        .update_style(data.name, category, data.description, data.style)
        .await?;
    res.render(Json(&updated));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    style.delete_style().await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
```

- [ ] **Step 2: Register the module**

In `src/api/mod.rs`, replace:

```rust
pub mod catalog;
pub mod categories;
pub mod database;
pub mod groups;
pub mod users;
```

with:

```rust
pub mod catalog;
pub mod categories;
pub mod database;
pub mod groups;
pub mod styles;
pub mod users;
```

- [ ] **Step 3: Wire the routes**

In `src/routes.rs`, add a new router-factory function right after
`build_api_categories_routes()` (added in Task 7):

```rust
fn build_api_styles_routes() -> Router {
    Router::with_path("styles")
        .get(api::styles::list)
        .post(api::styles::create)
        .push(
            Router::with_path("{id}")
                .put(api::styles::update)
                .delete(api::styles::delete),
        )
}
```

In `build_api_routes()`'s `admin` block, add it to the pushed list:

```rust
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .hoop(auth::validate_token)
                .hoop(auth::require_api_admin)
                .push(build_api_users_routes())
                .push(build_api_groups_routes())
                .push(build_api_categories_routes())
                .push(build_api_styles_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
        )
```

- [ ] **Step 4: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. Same rationale as Tasks 6/7 Step
4/5 — no isolated handler-level unit test; correctness rests on the
already-tested `Style` model methods (Tasks 2/3) plus this
build/regression check.

- [ ] **Step 5: Manual smoke check**

Same pattern as Task 6 Step 5, against `/api/admin/styles`. Use a body like:

```bash
curl -s -X POST http://localhost:5887/api/admin/styles \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"name": "smoke-test-style", "category": "<existing category id>", "description": "temp", "style": "{\"version\": 8, \"layers\": []}"}'
```

Confirm: create/list/update/delete all work, an invalid `style` JSON string
returns `400`, and — most importantly for Task 3 — after a create/update/
delete, hitting whatever public style-serving endpoint this app exposes
(check `src/services/styles.rs`) immediately reflects the change without a
server restart (proves the in-memory `STYLES` cache sync from Task 3 works
end-to-end through the new API surface, not just the HTML one).

- [ ] **Step 6: Commit**

```bash
git add src/api/styles.rs src/api/mod.rs src/routes.rs
git commit -m "feat(api): add Styles CRUD to the JSON admin API"
```

---

## Final verification

- [ ] Run `cargo test` one more time end-to-end — full green.
- [ ] Run `cargo clippy --all-targets` — no new warnings introduced by this
  plan's files (`src/error.rs`, `src/auth/models.rs`, `src/auth/tests.rs`,
  `src/models/category.rs`, `src/models/styles.rs`,
  `src/html/admin/styles.rs`, `src/api/groups.rs`, `src/api/categories.rs`,
  `src/api/styles.rs`, `src/api/mod.rs`, `src/routes.rs`).
- [ ] Confirm `src/main.rs` has no diff introduced by this plan (per the
  Global Constraints — it should only show whatever pre-existing,
  out-of-scope change was already on the branch before this plan started).
- [ ] Re-read the design spec
  (`docs/superpowers/specs/2026-07-28-admin-api-completion-phase2-design.md`)
  against the 8 tasks above and confirm each design section is covered:
  delete guard (Group) → Task 4, delete guard (Category) → Task 5, Style
  404 parity → Task 2, Style cache-sync parity → Task 3, `AppError::Conflict`
  → Task 1, Groups/Categories/Styles endpoints → Tasks 6/7/8.
