# Admin API Completion — Phase 1 (Bugs + Shared Infra) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the security/correctness bugs and build the cross-cutting
infrastructure (admin-role JWT gating, id/name request-shape convention,
`AppResult` standardization, shared cache invalidation) that the rest of the
admin API completion effort (Phase 2: Groups/Categories/Styles CRUD, Phase 3:
Users/Catalog CRUD completion) depends on.

**Architecture:** No new modules. Small, targeted edits to existing files:
`src/auth/models.rs`, `src/auth/handlers.rs`, `src/auth/mod.rs`,
`src/error.rs`, `src/api/users.rs`, `src/api/catalog.rs`,
`src/models/category.rs`, `src/api/database.rs`, `src/main.rs`,
`src/routes.rs`, `src/html/admin/catalog.rs`. Tests are colocated per the
project's existing convention (`#[cfg(test)] mod tests { ... }` inline at
the bottom of the file under test, using `#[test]` for pure-logic tests and
`#[tokio::test]` for async ones).

**Tech Stack:** Rust, Salvo 0.95 (`jwt-auth`, `test` features already
enabled), SQLx, serde/serde_json, jsonwebtoken, `thiserror`.

## Global Constraints

- All of `src/api/` returns `AppResult<T>` / `AppError` (project-wide
  standard per `CLAUDE.md`) — no `Result<T, StatusError>`, no manual
  `res.status_code(...)` + `return Err(...)` matches for new/changed code.
- Request bodies for layer/user/style writes take simple references
  (category by id, groups by name list), resolved server-side — never
  nested full `Category`/`Group` objects.
- Cache/reload logic is a shared function callable from both HTML and API
  handlers — never duplicated per surface.
- JWT admin claim is `groups: Vec<String>` (full group-name list), not a
  precomputed `is_admin: bool`.
- Only touch files this plan lists. Do not refactor unrelated code found
  along the way (e.g. leave `html/admin/catalog.rs`'s/`html/admin/users.rs`'s
  own group-resolution `filter_map` blocks as-is — only its cache
  invalidation block changes, per Task 8).
- Full design context: `docs/superpowers/specs/2026-07-27-admin-api-completion-phase1-design.md`.

---

### Task 1: Hide password hash from JSON responses

**Files:**
- Modify: `src/auth/models.rs:109-118` (the `User` struct)
- Test: `src/auth/tests.rs` (append to existing `mod tests`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `User` no longer serializes its `password` field. No signature
  changes — every caller of `User { password: ..., .. }` or `user.password`
  (field read) is unaffected; only `serde_json::to_*`/`Json(&user)` output
  changes.

- [ ] **Step 1: Write the failing test**

Open `src/auth/tests.rs`. Add this test inside the existing `mod tests { ... }`
block (anywhere after the `use` statements, e.g. right after
`create_test_user`):

```rust
    #[test]
    fn test_user_password_not_serialized() {
        let user = User {
            id: "1".to_string(),
            username: "test".to_string(),
            email: "test@test.com".to_string(),
            first_name: None,
            last_name: None,
            password: "supersecrethash".to_string(),
            groups: vec![],
        };

        let value = serde_json::to_value(&user).unwrap();
        assert!(value.get("password").is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_user_password_not_serialized -- --nocapture`
Expected: FAIL — `value.get("password")` is `Some(...)`, assertion fails.

- [ ] **Step 3: Add `#[serde(skip_serializing)]` to `User.password`**

In `src/auth/models.rs`, find:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub password: String,
    pub groups: Vec<Group>,
}
```

Change the `password` field to:

```rust
    #[serde(skip_serializing)]
    pub password: String,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_user_password_not_serialized -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run the full existing test suite to confirm no regressions**

Run: `cargo test`
Expected: all tests pass (in particular `src/auth/tests.rs`'s existing
password/serialization-adjacent tests, and `src/config/users.rs`, which
constructs `User` from raw SQL rows, not from JSON — unaffected by this
change).

- [ ] **Step 6: Commit**

```bash
git add src/auth/models.rs src/auth/tests.rs
git commit -m "fix(auth): stop serializing password hash in User JSON output"
```

---

### Task 2: Add `Auth::resolve_groups_by_name` shared resolver

**Files:**
- Modify: `src/auth/models.rs` (add a method on `impl Auth`, near
  `find_group_by_name` at line 178)
- Test: `src/auth/tests.rs`

**Interfaces:**
- Consumes: `Auth.groups: Vec<Group>`, `Auth::find_group_by_name` (existing).
- Produces: `Auth::resolve_groups_by_name(&self, names: &[String]) -> Vec<Group>`
  — used by Task 3 (`api::users::create`) and Task 4
  (`api::catalog::create_layer`). Unknown names are silently dropped
  (matches the existing `html/admin/catalog.rs::create_layer` behavior).

- [ ] **Step 1: Write the failing tests**

Add to `src/auth/tests.rs`, inside `mod tests`:

```rust
    #[test]
    fn test_resolve_groups_by_name_matches_known_drops_unknown() {
        let auth = create_test_auth();
        let resolved = auth.resolve_groups_by_name(&[
            "admin".to_string(),
            "ghost".to_string(),
            "users".to_string(),
        ]);
        let mut names: Vec<String> = resolved.iter().map(|g| g.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["admin", "users"]);
    }

    #[test]
    fn test_resolve_groups_by_name_empty_input() {
        let auth = create_test_auth();
        assert!(auth.resolve_groups_by_name(&[]).is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test resolve_groups_by_name -- --nocapture`
Expected: FAIL with "no method named `resolve_groups_by_name`".

- [ ] **Step 3: Implement the method**

In `src/auth/models.rs`, right after `find_group_by_name` (currently at
line 178-180):

```rust
    pub fn find_group_by_name<'a>(&'a self, target_name: &'a str) -> Option<&'a Group> {
        self.groups.iter().find(|m| m.name == target_name)
    }

    pub fn resolve_groups_by_name(&self, names: &[String]) -> Vec<Group> {
        names
            .iter()
            .filter_map(|name| self.find_group_by_name(name).cloned())
            .collect()
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test resolve_groups_by_name -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/auth/models.rs src/auth/tests.rs
git commit -m "feat(auth): add Auth::resolve_groups_by_name shared resolver"
```

---

### Task 3: Fix `api::users::create` to resolve group names instead of discarding them

**Files:**
- Modify: `src/api/users.rs:15-22` (the `NewUser` struct) and `:61-81`
  (the `create` handler)

**Interfaces:**
- Consumes: `Auth::resolve_groups_by_name` (Task 2).
- Produces: `POST /api/admin/users` now honors the `groups` field it
  accepts (list of group names) instead of silently discarding it. Request
  body shape change: `groups: Vec<Option<Group>>` → `groups: Option<Vec<String>>`.

- [ ] **Step 1: Change the `NewUser` request DTO**

In `src/api/users.rs`, replace:

```rust
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: String,
    groups: Vec<Option<Group>>,
}
```

with:

```rust
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: String,
    groups: Option<Vec<String>>,
}
```

- [ ] **Step 2: Resolve the names in the `create` handler**

Replace:

```rust
#[handler]
pub async fn create<'a>(res: &mut Response, data: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;
    let encrypt_psw = auth
        .get_encrypt_psw(data.password.to_string())
        .map_err(AppError::PasswordHashError)?;

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: data.username.to_string(),
        email: data.email,
        first_name: data.first_name,
        last_name: data.last_name,
        password: encrypt_psw,
        groups: Vec::new(),
    };

    auth.create_user(user.clone()).await?;
    res.render(Json(&user));
    Ok(())
}
```

with:

```rust
#[handler]
pub async fn create<'a>(res: &mut Response, data: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;
    let encrypt_psw = auth
        .get_encrypt_psw(data.password.to_string())
        .map_err(AppError::PasswordHashError)?;

    let groups = data
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: data.username.to_string(),
        email: data.email,
        first_name: data.first_name,
        last_name: data.last_name,
        password: encrypt_psw,
        groups,
    };

    auth.create_user(user.clone()).await?;
    res.render(Json(&user));
    Ok(())
}
```

Note: `Group` is no longer referenced directly in this file's DTO, but it's
still used transitively — leave the `use crate::auth::{..., Group, ...};`
import as-is (`User` construction still needs it in scope for `groups:
Vec<Group>`'s type to resolve through `resolve_groups_by_name`'s return
type). If `cargo build` reports it unused, remove it then — don't
pre-emptively remove it.

- [ ] **Step 3: Verify it builds and the full suite still passes**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. (This handler touches `get_auth()`
global state and persists to SQLite via `Auth::create_user`, so it isn't
unit-testable in isolation without a full app bootstrap — consistent with
every other handler in `src/api/` and `src/html/admin/`, none of which have
direct handler-level tests today. Its correctness rests on the
already-tested `Auth::resolve_groups_by_name` from Task 2 plus this
build/regression check.)

- [ ] **Step 4: Commit**

```bash
git add src/api/users.rs
git commit -m "fix(api): resolve group names in api::users::create instead of discarding them"
```

---

### Task 4: Fix `api::catalog::create_layer` to use id/name references instead of nested objects

**Files:**
- Modify: `src/models/category.rs:34-38` (`Category::from_id` — map
  "not found" to a proper 404 instead of a raw `sqlx::Error`)
- Modify: `src/api/catalog.rs` (replace `create_layer`)
- Test: `src/models/category.rs` (new `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Auth::resolve_groups_by_name` (Task 2), `Category::from_id`
  (existing, behavior tightened by Step 1).
- Produces: `POST /api/admin/catalog/layer` now takes `category` as an id
  string and `groups` as a list of names (matching `html/admin/catalog.rs`'s
  `NewLayer` shape) instead of requiring the client to send full nested
  `Category`/`Group` JSON objects. Unknown category id → 404
  (`AppError::NotFound`), not 500.

- [ ] **Step 1: Write a regression test for the precondition `Category::from_id` relies on**

`Category::from_id` calls `get_category_by_id(None, id)`, and `None` means
"use the global config pool" (`get_cf_pool()`, a `OnceLock` uninitialized
in unit tests) — so `Category::from_id` itself can't be called in
isolation here, the same global-state limit as other tasks in this plan.
This step does **not** TDD-drive Step 3's change (there's no way to make
`Category::from_id` fail-then-pass without global init); it locks down the
`sqlx::Error::RowNotFound` behavior that Step 3's `match` arm depends on,
using the pool-injectable `get_category_by_id` directly, so a future sqlx
upgrade that changed this behavior would be caught here.

`src/models/category.rs` has no test module yet. Add one at the bottom of
the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn get_category_by_id_returns_row_not_found_for_unknown_id() {
        let pool = in_memory_pool().await;
        let result = crate::config::categories::get_category_by_id(Some(&pool), "missing-id").await;
        assert!(matches!(result, Err(sqlx::Error::RowNotFound)));
    }
}
```

- [ ] **Step 2: Run it to confirm it passes**

Run: `cargo test get_category_by_id_returns_row_not_found_for_unknown_id -- --nocapture`
Expected: PASS (this is a regression guard, not a failing-first test — see
Step 1's note).

- [ ] **Step 3: Map `RowNotFound` to `AppError::NotFound` in `Category::from_id`**

In `src/models/category.rs`, change the top import from:

```rust
use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::AppResult,
    get_catalog, get_categories,
};
```

to:

```rust
use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::{AppError, AppResult},
    get_catalog, get_categories,
};
```

Then replace:

```rust
    pub async fn from_id(id: &str) -> AppResult<Self> {
        let category = get_category_by_id(None, id).await?;

        Ok(category)
    }
```

with:

```rust
    pub async fn from_id(id: &str) -> AppResult<Self> {
        match get_category_by_id(None, id).await {
            Ok(category) => Ok(category),
            Err(sqlx::Error::RowNotFound) => {
                Err(AppError::NotFound(format!("Category {id} not found")))
            }
            Err(e) => Err(e.into()),
        }
    }
```

- [ ] **Step 4: Build to confirm no regressions in existing callers**

Run: `cargo build && cargo test`
Expected: builds clean (existing callers — `html/admin/catalog.rs`,
`html/admin/styles.rs` — already handle an `Err` generically via `?`/match,
so a different `AppError` variant doesn't break them). All tests pass.

- [ ] **Step 5: Replace `create_layer`'s request shape in `src/api/catalog.rs`**

Replace the entire file contents with:

```rust
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth, get_catalog,
    models::{catalog::Layer, category::Category},
};

#[handler]
pub async fn list(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let catalog = get_catalog().await.read().await;
    let mut layers = catalog.layers.clone();
    let scheme = req.scheme().to_string();

    let host = req
        .headers()
        .get("host")
        .ok_or(AppError::RequestParamError("Missing host header".to_string()))?
        .to_str()
        .map_err(|_| AppError::RequestParamError("Invalid host header encoding".to_string()))?;

    for layer in &mut layers {
        layer.url = Some(format!(
            "{scheme}://{host}/services/tiles/{}:{}/{{z}}/{{x}}]/{{y}}].pbf",
            layer.category.name, layer.name
        ));
    }

    res.render(Json(&layers));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewLayerRequest {
    category: String,
    database_id: String,
    geometry: String,
    name: String,
    alias: String,
    description: String,
    schema: String,
    table: String,
    fields: Vec<String>,
    filter: Option<String>,
    srid: Option<u32>,
    geom: Option<String>,
    sql_mode: Option<String>,
    buffer: Option<u32>,
    extent: Option<u32>,
    zmin: Option<u32>,
    zmax: Option<u32>,
    zmax_do_not_simplify: Option<u32>,
    buffer_do_not_simplify: Option<u32>,
    extent_do_not_simplify: Option<u32>,
    clip_geom: Option<bool>,
    delete_cache_on_start: Option<bool>,
    max_cache_age: Option<u64>,
    max_records: Option<u64>,
    published: bool,
    groups: Option<Vec<String>>,
}

#[handler]
pub async fn create_layer(res: &mut Response, layer_form: NewLayerRequest) -> AppResult<()> {
    let category = Category::from_id(&layer_form.category).await?;

    let auth = get_auth().await.read().await;
    let groups: Vec<Group> = layer_form
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();
    drop(auth);

    let name = crate::services::utils::normalize_name(&layer_form.name)?;

    let layer = Layer {
        id: uuid::Uuid::new_v4().simple().to_string(),
        category,
        database_id: layer_form.database_id,
        geometry: layer_form.geometry,
        name,
        alias: layer_form.alias,
        description: layer_form.description,
        schema: layer_form.schema,
        table_name: layer_form.table,
        fields: layer_form.fields,
        filter: layer_form.filter,
        srid: layer_form.srid,
        geom: layer_form.geom,
        sql_mode: layer_form.sql_mode,
        buffer: layer_form.buffer,
        extent: layer_form.extent,
        zmin: layer_form.zmin,
        zmax: layer_form.zmax,
        zmax_do_not_simplify: layer_form.zmax_do_not_simplify,
        buffer_do_not_simplify: layer_form.buffer_do_not_simplify,
        extent_do_not_simplify: layer_form.extent_do_not_simplify,
        clip_geom: layer_form.clip_geom,
        delete_cache_on_start: layer_form.delete_cache_on_start,
        max_cache_age: layer_form.max_cache_age,
        max_records: layer_form.max_records,
        published: layer_form.published,
        url: None,
        groups: Some(groups),
    };

    let mut catalog = get_catalog().await.write().await;
    catalog.add_layer(layer.clone()).await?;
    res.render(Json(&layer));
    Ok(())
}
```

This adds a `uuid` direct dependency use (`uuid::Uuid`) — check
`Cargo.toml` already lists `uuid` as a dependency (it does; `src/api/users.rs`
already does `use uuid::Uuid;`).

- [ ] **Step 6: Build to confirm it compiles**

Run: `cargo build`
Expected: builds clean. If `Group` import is reported unused (it's used
only via `Vec<Group>`'s turbofish-free inference, which does need the type
in scope for the `groups: Vec<Group>` binding's annotation) — keep it; it's
used.

- [ ] **Step 7: Run full suite**

Run: `cargo test`
Expected: all tests pass, including the new
`get_category_by_id_returns_row_not_found_for_unknown_id`.

- [ ] **Step 8: Commit**

```bash
git add src/models/category.rs src/api/catalog.rs
git commit -m "fix(api): create_layer takes category id + group names instead of nested objects"
```

---

### Task 5: Add `groups` claim to `JwtClaims`

**Files:**
- Modify: `src/auth/models.rs:19-25` (`JwtClaims` struct), `:271-291`
  (`Auth::login`)
- Test: `src/auth/tests.rs`

**Interfaces:**
- Consumes: `User::groups_as_vec_string()` (existing).
- Produces: `JwtClaims.groups: Vec<String>`,
  `JwtClaims::is_admin(&self) -> bool` — used by Task 6's
  `require_api_admin`.

- [ ] **Step 1: Write the failing tests**

Add to `src/auth/tests.rs`:

```rust
    #[test]
    fn test_jwtclaims_is_admin_true() {
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "admin".to_string(),
            email: "admin@test.com".to_string(),
            groups: vec!["admin".to_string()],
            exp: 0,
        };
        assert!(claims.is_admin());
    }

    #[test]
    fn test_jwtclaims_is_admin_false() {
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "regular".to_string(),
            email: "regular@test.com".to_string(),
            groups: vec!["users".to_string()],
            exp: 0,
        };
        assert!(!claims.is_admin());
    }

    #[test]
    fn test_jwtclaims_is_admin_empty_groups() {
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "nogroups".to_string(),
            email: "nogroups@test.com".to_string(),
            groups: vec![],
            exp: 0,
        };
        assert!(!claims.is_admin());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test jwtclaims_is_admin -- --nocapture`
Expected: FAIL to compile — `JwtClaims` has no `groups` field yet, and no
`is_admin` method.

- [ ] **Step 3: Add the `groups` field and `is_admin` method**

In `src/auth/models.rs`, replace:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub id: String,
    pub username: String,
    pub email: String,
    pub exp: i64,
}
```

with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub id: String,
    pub username: String,
    pub email: String,
    pub groups: Vec<String>,
    pub exp: i64,
}

impl JwtClaims {
    pub fn is_admin(&self) -> bool {
        self.groups.iter().any(|g| g == "admin")
    }
}
```

- [ ] **Step 4: Populate `groups` in `Auth::login`**

Find (currently around line 271-291):

```rust
    pub fn login(&mut self, email: &str, psw: &str) -> AppResult<String> {
        let jwt_secret = get_jwt_secret();
        for user in self.users.clone().into_iter() {
            if email == user.email && self.validate_psw(user.clone(), psw)? {
                let exp = OffsetDateTime::now_utc() + Duration::days(1);
                let claim = JwtClaims {
                    id: user.id,
                    username: user.username.to_owned(),
                    email: email.to_owned(),
                    exp: exp.unix_timestamp(),
                };
                let token = jsonwebtoken::encode(
                    &jsonwebtoken::Header::default(),
                    &claim,
                    &EncodingKey::from_secret(jwt_secret.as_bytes()),
                )?;
                return Ok(token);
            }
        }
        Ok("".to_owned())
    }
```

Replace with (note `groups` is computed *before* `user.id` is moved into
the claim literal, since `groups_as_vec_string(&self)` needs an intact
`user`):

```rust
    pub fn login(&mut self, email: &str, psw: &str) -> AppResult<String> {
        let jwt_secret = get_jwt_secret();
        for user in self.users.clone().into_iter() {
            if email == user.email && self.validate_psw(user.clone(), psw)? {
                let exp = OffsetDateTime::now_utc() + Duration::days(1);
                let groups = user.groups_as_vec_string();
                let claim = JwtClaims {
                    id: user.id,
                    username: user.username.to_owned(),
                    email: email.to_owned(),
                    groups,
                    exp: exp.unix_timestamp(),
                };
                let token = jsonwebtoken::encode(
                    &jsonwebtoken::Header::default(),
                    &claim,
                    &EncodingKey::from_secret(jwt_secret.as_bytes()),
                )?;
                return Ok(token);
            }
        }
        Ok("".to_owned())
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test jwtclaims_is_admin -- --nocapture`
Expected: PASS

- [ ] **Step 6: Run full suite**

Run: `cargo test`
Expected: all pass. `cargo build` must also succeed — grep confirms no
other `JwtClaims { ... }` struct literals exist outside `Auth::login`, so
this is the only construction site needing an update.

- [ ] **Step 7: Commit**

```bash
git add src/auth/models.rs src/auth/tests.rs
git commit -m "feat(auth): add groups claim to JwtClaims for API admin gating"
```

---

### Task 6: Add `AppError::Forbidden` + `require_api_admin` + wire into routes

**Files:**
- Modify: `src/error.rs` (add `Forbidden` variant)
- Modify: `src/auth/handlers.rs` (add `require_api_admin`)
- Modify: `src/auth/mod.rs` (export it)
- Modify: `src/routes.rs:251-256` (`build_api_routes`)
- Test: `src/auth/handlers.rs` (new `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `JwtClaims::is_admin()` (Task 5).
- Produces: `auth::require_api_admin` handler, wired as a `.hoop()` on the
  `admin` sub-router in `build_api_routes()` — automatically covers every
  route Phase 2/3 add under that sub-router too.

- [ ] **Step 1: Add the `Forbidden` variant to `AppError`**

In `src/error.rs`, find:

```rust
    #[error("Unauthorized access")]
    UnauthorizedAccess,
```

Add right after it:

```rust
    #[error("Unauthorized access")]
    UnauthorizedAccess,

    #[error("Forbidden: {0}")]
    Forbidden(String),
```

Then in `status_code`, find:

```rust
            Self::UnauthorizedAccess | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
```

Add right after it:

```rust
            Self::UnauthorizedAccess | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
```

- [ ] **Step 2: Write the failing integration tests for `require_api_admin`**

In `src/auth/handlers.rs`, append at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use salvo::test::TestClient;
    use time::{Duration, OffsetDateTime};

    fn sign_token(groups: Vec<String>) -> String {
        let _ = crate::JWT_SECRET.set("test-only-secret-not-used-in-prod".to_string());
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "tester".to_string(),
            email: "tester@test.com".to_string(),
            groups,
            exp: (OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(crate::get_jwt_secret().as_bytes()),
        )
        .unwrap()
    }

    fn protected_router() -> Router {
        #[handler]
        async fn ok(res: &mut Response) {
            res.render("ok");
        }

        Router::new()
            .hoop(jwt_auth_handler())
            .hoop(require_api_admin)
            .get(ok)
    }

    #[tokio::test]
    async fn require_api_admin_allows_admin_token() {
        let service = Service::new(protected_router());
        let token = sign_token(vec!["admin".to_string()]);
        let res = TestClient::get("http://127.0.0.1:5800/")
            .bearer_auth(token)
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_api_admin_rejects_non_admin_token() {
        let service = Service::new(protected_router());
        let token = sign_token(vec!["users".to_string()]);
        let res = TestClient::get("http://127.0.0.1:5800/")
            .bearer_auth(token)
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn require_api_admin_rejects_missing_token() {
        let service = Service::new(protected_router());
        let res = TestClient::get("http://127.0.0.1:5800/").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test require_api_admin -- --nocapture`
Expected: FAIL to compile — `require_api_admin` doesn't exist yet.

- [ ] **Step 4: Implement `require_api_admin`**

In `src/auth/handlers.rs`, add right after `require_user_admin` (currently
lines 36-51):

```rust
#[handler]
pub async fn require_api_admin(depot: &mut Depot) -> AppResult<()> {
    let is_admin = depot
        .jwt_auth_data::<JwtClaims>()
        .is_some_and(|data| data.claims.is_admin());

    if is_admin {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin privileges required".to_string()))
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test require_api_admin -- --nocapture`
Expected: PASS

- [ ] **Step 6: Export `require_api_admin` from `src/auth/mod.rs`**

In `src/auth/mod.rs`, change:

```rust
pub use handlers::{
    change_password, jwt_auth_handler, login, logout, require_user_admin, session_auth_handler,
    validate_token,
};
```

to:

```rust
pub use handlers::{
    change_password, jwt_auth_handler, login, logout, require_api_admin, require_user_admin,
    session_auth_handler, validate_token,
};
```

- [ ] **Step 7: Wire it into `build_api_routes()`**

In `src/routes.rs`, find (currently lines 241-257):

```rust
fn build_api_routes() -> Router {
    Router::with_path("api")
        .push(
            Router::with_path("users/login")
                .hoop(build_login_rate_limiter())
                .post(api::users::login),
        )
        .push(Router::with_path("monitor/metrics").get(monitor::handlers::metrics))
        .push(Router::with_path("catalog/layer").get(api::catalog::list))
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .push(build_api_users_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
        )
}
```

Replace the `admin` sub-router push with:

```rust
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .hoop(auth::require_api_admin)
                .push(build_api_users_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
        )
```

- [ ] **Step 8: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/error.rs src/auth/handlers.rs src/auth/mod.rs src/routes.rs
git commit -m "feat(api): gate /api/admin routes behind JWT admin-group check"
```

---

### Task 7: Standardize `src/api/database.rs` on `AppResult`

**Files:**
- Modify: `src/api/database.rs` (entire file)

**Interfaces:**
- Consumes: `query_schemas`, `query_tables`, `query_fields`, `query_srid`
  (existing, already `AppResult`-returning).
- Produces: no signature change visible to routing (`src/routes.rs` calls
  these the same way); only the handlers' internal error type changes from
  `Result<Json<T>, StatusError>` to `AppResult<Json<T>>`.

- [ ] **Step 1: Replace the file**

Replace the full contents of `src/api/database.rs` with:

```rust
use salvo::prelude::*;

use crate::db::metadata::{
    Field, Schema, Srid, Table, query_fields, query_schemas, query_srid, query_tables,
};
use crate::error::AppResult;

#[handler]
pub async fn schemas(req: &mut Request) -> AppResult<Json<Vec<Schema>>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    Ok(Json(query_schemas(&db_id).await?))
}

#[handler]
pub async fn tables(req: &mut Request) -> AppResult<Json<Vec<Table>>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    Ok(Json(query_tables(&db_id, schema).await?))
}

#[handler]
pub async fn fields(req: &mut Request) -> AppResult<Json<Vec<Field>>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    Ok(Json(query_fields(&db_id, schema, table).await?))
}

#[handler]
pub async fn srid(req: &mut Request) -> AppResult<Json<Srid>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    let geometry = req.query::<String>("geometry").unwrap_or_default();
    Ok(Json(query_srid(&db_id, schema, table, geometry).await?))
}
```

Note: this drops the per-handler `tracing::error!("{}", e);` calls. That
matches how every other `AppResult`-returning handler in the codebase
behaves (none manually log before propagating `?`) — the project doesn't
have a centralized error-logging layer beyond the `Logger::default()`
request-logging hoop already mounted in `routes.rs`. This is an intentional
behavior change, not an oversight.

Also note the status code for a DB failure changes: previously a hand-rolled
`StatusError::bad_request()` (400) regardless of the actual failure;
now `AppError`'s default mapping (500 for `SQLError`/unclassified errors) —
correcting a misleading 400 into an accurate 500. This is an intentional,
documented status-code change (see the spec's Testing section).

- [ ] **Step 2: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. (These handlers hit
`get_db_registry()` — a live-Postgres-backed global — so, consistent with
the rest of `src/api/` and `src/html/admin/`, there's no isolated unit test
for the handlers themselves; correctness rests on the already-`AppResult`
`query_*` functions being unchanged, plus this build/regression check.)

- [ ] **Step 3: Commit**

```bash
git add src/api/database.rs
git commit -m "refactor(api): standardize database.rs handlers on AppResult"
```

---

### Task 8: Extract shared tile-cache invalidation into `invalidate_layer_tile_cache`

**Files:**
- Modify: `src/main.rs` (add function near `reload_styles_cache`, currently
  around line 124)
- Modify: `src/html/admin/catalog.rs` (`update_layer`, currently lines
  209-306, and its top `use` block)

**Interfaces:**
- Consumes: `get_cache_invalidation_delay()`, `get_cache_wrapper()`
  (existing globals in `src/main.rs`).
- Produces: `pub async fn invalidate_layer_tile_cache(layer_key: &str) -> AppResult<()>`
  in the crate root — callable from any handler (HTML now, API in Phase 3)
  via `crate::invalidate_layer_tile_cache(...)`.

- [ ] **Step 1: Add the shared function in `src/main.rs`**

Right after `reload_styles_cache` (currently lines 124-128):

```rust
pub async fn reload_styles_cache() -> AppResult<()> {
    let styles = config::styles::get_styles(Some(get_cf_pool())).await?;
    *get_styles_cache().await.write().await = styles;
    Ok(())
}

/// Invalidates a layer's tile cache exactly like the manual "clear cache"
/// action: bumps the layer version (so ETags change and clients/QGIS
/// refetch) and removes stale tiles. In clustered owner/shared modes
/// (`get_cache_invalidation_delay()` returns `Some`), the clear is deferred
/// so peers have time to reload the already-bumped config before the
/// shared cache is wiped — otherwise a lagging peer could repopulate it
/// with a stale tile.
pub async fn invalidate_layer_tile_cache(layer_key: &str) -> AppResult<()> {
    let key = layer_key.to_string();
    match get_cache_invalidation_delay() {
        Some(delay) => {
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                if let Err(e) = get_cache_wrapper().delete_layer_cache(&key).await {
                    tracing::warn!("deferred cache invalidation failed for {key}: {e}");
                }
            });
        }
        None => {
            get_cache_wrapper().delete_layer_cache(&key).await?;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Use it from `html/admin/catalog.rs::update_layer`**

In `src/html/admin/catalog.rs`, find the block (currently lines 276-300):

```rust
    // The layer config changed (fields, filter, sql_mode, ...): invalidate its
    // tile cache exactly like the manual "clear cache" action. This bumps the
    // layer version so the ETag changes (forcing browsers/QGIS to refetch) and
    // removes the stale tiles, so the next request regenerates them from the DB
    // with the updated columns.
    //
    // In clustered owner/shared modes the invalidation is deferred: update_layer
    // already bumped config_version, so peers reload the new config within their
    // watch interval; clearing the shared cache only after that elapses prevents
    // a lagging peer from repopulating it with a stale tile.
    match get_cache_invalidation_delay() {
        Some(delay) => {
            let key = layer_key.clone();
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                if let Err(e) = get_cache_wrapper().delete_layer_cache(&key).await {
                    tracing::warn!("deferred cache invalidation failed for {key}: {e}");
                }
            });
        }
        None => {
            get_cache_wrapper().delete_layer_cache(&layer_key).await?;
        }
    }
```

Replace it with:

```rust
    // The layer config changed (fields, filter, sql_mode, ...): invalidate
    // its tile cache exactly like the manual "clear cache" action, so the
    // next request regenerates tiles from the DB with the updated columns.
    // See `invalidate_layer_tile_cache` for the cluster deferred-delay
    // rationale.
    crate::invalidate_layer_tile_cache(&layer_key).await?;
```

- [ ] **Step 3: Remove the now-unused imports**

In `src/html/admin/catalog.rs`, find the top `use` block:

```rust
use crate::{
    auth::{Group, User},
    error::{AppError, AppResult},
    get_auth, get_cache_invalidation_delay, get_cache_wrapper, get_catalog, get_categories,
    get_db_registry,
    html::utils::{BaseTemplateData, make_base},
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
    },
};
```

Replace with:

```rust
use crate::{
    auth::{Group, User},
    error::{AppError, AppResult},
    get_auth, get_catalog, get_categories, get_db_registry,
    html::utils::{BaseTemplateData, make_base},
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
    },
};
```

- [ ] **Step 4: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. This is a pure code-move (no logic
change) — `get_cache_wrapper()`/`get_cache_invalidation_delay()` are
process-global statics not swappable in unit tests, so, consistent with the
rest of the cache/catalog code, there's no new isolated unit test; the
guarantee here is the diff being a mechanical extraction plus the
build/regression check.

- [ ] **Step 5: Manual smoke check**

Run: `cargo run` (or `cargo run -- --config <path>` per `CLAUDE.md`), log
in to `/admin/catalog`, edit an existing layer's field list or filter, save,
and confirm (via server logs or by requesting a tile for that layer) the
cache still gets invalidated exactly as before — no behavior change
expected, this just confirms the refactor didn't silently break the call
path.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/html/admin/catalog.rs
git commit -m "refactor: extract shared invalidate_layer_tile_cache from html/admin/catalog"
```

---

## Final verification

- [ ] Run `cargo test` one more time end-to-end — full green.
- [ ] Run `cargo build --release` to confirm no release-profile-only
  warnings/errors were introduced.
- [ ] Re-read the design spec
  (`docs/superpowers/specs/2026-07-27-admin-api-completion-phase1-design.md`)
  Components 1-6 against the 8 tasks above and confirm each is covered:
  1 → Task 1, 2 → Task 3, 3 → Task 4, 4 → Tasks 5+6, 5 → Task 7, 6 → Task 8.
  (Task 2, `Auth::resolve_groups_by_name`, is new shared infra the spec's
  Component 2/3 implied but didn't name explicitly — it's what makes
  Tasks 3 and 4 both correct and independently testable.)
