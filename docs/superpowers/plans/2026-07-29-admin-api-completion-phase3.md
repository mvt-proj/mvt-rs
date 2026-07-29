# Admin API Completion — Phase 3 (Users update/delete, Catalog update/delete/publish/cache-delete) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the two remaining partial JSON REST API surfaces —
`src/api/users.rs` (only login/list/create) and `src/api/catalog.rs` (only
list/create) — by adding update/delete for Users and update/delete/publish-
toggle/cache-clear for Catalog layers, reaching parity with the HTML admin
panel.

**Architecture:** Small, additive edits to two existing handler files
(`src/api/users.rs`, `src/api/catalog.rs`) plus their route wiring in
`src/routes.rs`. One small pure-function extraction in `src/auth/models.rs`
(`resolve_updated_password`) so the update DTO's password semantics are
genuinely unit-testable without the global `Auth` `OnceLock`. No model-layer
mutation methods (`Auth::update_user`, `Auth::delete_user`,
`Catalog::update_layer`, `Catalog::delete_layer`,
`Catalog::swich_layer_published`) are modified — each new handler adds its
own existence pre-check before calling them.

**Tech Stack:** Rust, Salvo 0.95 (`Extractible` derive with per-field
`source(from = "param")` override for path params in a body-sourced
struct), SQLx, serde/serde_json.

## Global Constraints

- All of `src/api/` returns `AppResult<T>` / `AppError` — no manual status
  codes, no `Result<T, StatusError>`.
- Update endpoints are full replacement (all editable fields required in
  the body) — no partial/`PATCH`-style field updates. (`PATCH` as an HTTP
  verb is still used for the `publish` toggle route — that's an action
  endpoint with no body, not a partial field update, so it doesn't violate
  this constraint.)
- No self-delete or last-admin delete guard for Users — exact parity with
  the HTML admin's current (guard-less) behavior.
- `password: Option<String>` in the user update DTO. `None` (omitted/null)
  means keep the existing hash; `Some(pw)` means set a new password.
- Existence pre-checks (`AppError::NotFound` on a missing id) are added only
  in the new API handlers. Do **not** modify `Auth::update_user`,
  `Auth::delete_user`, `Catalog::update_layer`, `Catalog::delete_layer`, or
  `Catalog::swich_layer_published` — their current silent-no-op-on-missing-id
  behavior must keep serving the HTML admin unchanged.
- No tile-cache invalidation on layer delete — matches the HTML
  `delete_layer` handler's current behavior (pre-existing gap, not fixed
  here). Layer *update* does still invalidate the cache (unchanged parity
  with the HTML `update_layer` handler).
- `Style.category`-style "reference by id" pattern doesn't apply here
  (Users/Catalog don't reference Styles); Catalog's `category` field is
  still referenced by id via `Category::from_id`, matching the existing
  `create_layer` handler.
- Only touch files this plan lists: `src/api/users.rs`, `src/api/catalog.rs`,
  `src/auth/models.rs`, `src/auth/tests.rs`, `src/routes.rs`.
- Full design context: `docs/superpowers/specs/2026-07-29-admin-api-completion-phase3-design.md`.

---

### Task 1: Users API — `update` and `delete`

**Files:**
- Modify: `src/auth/models.rs` (new pure function `resolve_updated_password`)
- Modify: `src/auth/tests.rs` (tests for the new pure function)
- Modify: `src/api/users.rs` (`update`, `delete` handlers)
- Modify: `src/routes.rs` (`build_api_users_routes`)

**Interfaces:**
- Consumes: `Auth::get_user_by_id(&self, id: &str) -> Option<&User>`,
  `Auth::resolve_groups_by_name(&self, names: &[String]) -> Vec<Group>`,
  `Auth::get_encrypt_psw(&self, psw: String) -> Result<String, argon2::password_hash::Error>`,
  `Auth::update_user(&mut self, user: User) -> AppResult<()>`,
  `Auth::delete_user(&mut self, id: String) -> AppResult<()>` (all existing,
  unchanged).
- Produces: `resolve_updated_password(auth: &Auth, existing_password: &str, new_password: Option<String>) -> AppResult<String>`
  (pure-ish: only touches `self` via the already-side-effect-free
  `get_encrypt_psw`, so it's callable with any `Auth` value — no global
  state needed). `PUT/DELETE /api/admin/users/{id}`.

- [ ] **Step 1: Write the failing tests for `resolve_updated_password`**

Add to `src/auth/tests.rs`'s existing `mod tests { ... }` block, after the
existing `create_test_user` helper. Extend the `use` line that currently
imports `count_group_references`:

```rust
    use super::super::models::{
        Auth, Group, JwtClaims, User, count_group_references, resolve_updated_password,
    };
```

Then add the tests (place them near the other `Auth`-focused tests in this
file):

```rust
    #[test]
    fn test_resolve_updated_password_none_keeps_existing_hash() {
        let auth = create_test_auth();
        let result = resolve_updated_password(&auth, "existing-hash", None).unwrap();
        assert_eq!(result, "existing-hash");
    }

    #[test]
    fn test_resolve_updated_password_some_hashes_the_new_password() {
        let auth = create_test_auth();
        let result =
            resolve_updated_password(&auth, "existing-hash", Some("new-password".to_string()))
                .unwrap();
        assert_ne!(result, "existing-hash");
        assert_ne!(result, "new-password"); // stored as an argon2 hash, not plaintext
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test resolve_updated_password -- --nocapture`
Expected: FAIL to compile — `resolve_updated_password` doesn't exist yet.

- [ ] **Step 3: Implement the pure function**

In `src/auth/models.rs`, add right after `Auth::get_encrypt_psw`'s closing
`}` (currently ending at line 237, inside `impl Auth`) — this needs to be a
free function, not a method, so it can take `&Auth` as a plain parameter and
sit alongside `count_group_references`:

Find:

```rust
    pub fn get_encrypt_psw(&self, psw: String) -> Result<String, argon2::password_hash::Error> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(psw.as_bytes(), &salt)?.to_string();
        Ok(password_hash)
    }
```

Leave that method as-is. Instead, add the free function next to
`count_group_references` (after its closing `}`, before the `User` struct
definition):

```rust
/// Resolves the password hash to persist on a user update. `None` (the
/// password field omitted from the request) keeps the existing hash;
/// `Some(pw)` hashes the new password. Takes `&Auth` (rather than being an
/// `Auth` method) purely so call sites read naturally next to
/// `auth.get_encrypt_psw`; it doesn't touch any `Auth` field, which is what
/// makes it unit-testable with any `Auth` value, no global state needed.
pub fn resolve_updated_password(
    auth: &Auth,
    existing_password: &str,
    new_password: Option<String>,
) -> AppResult<String> {
    match new_password {
        Some(pw) => auth.get_encrypt_psw(pw).map_err(AppError::PasswordHashError),
        None => Ok(existing_password.to_string()),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test resolve_updated_password -- --nocapture`
Expected: PASS

- [ ] **Step 5: Add the `update` and `delete` handlers**

In `src/api/users.rs`, the file currently ends with the `create` handler.
Add these two handlers at the end of the file:

```rust
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateUser<'a> {
    #[salvo(extract(source(from = "param")))]
    id: String,
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: Option<String>,
    groups: Option<Vec<String>>,
}

#[handler]
pub async fn update<'a>(res: &mut Response, data: UpdateUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;

    let existing = auth
        .get_user_by_id(&data.id)
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", data.id)))?
        .clone();

    let password =
        crate::auth::models::resolve_updated_password(&auth, &existing.password, data.password)?;

    let groups = data
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();

    let user = User {
        id: data.id,
        username: data.username.to_string(),
        email: data.email,
        first_name: data.first_name,
        last_name: data.last_name,
        password,
        groups,
    };

    auth.update_user(user.clone()).await?;
    res.render(Json(&user));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let mut auth = get_auth().await.write().await;
    if auth.get_user_by_id(&id).is_none() {
        return Err(AppError::NotFound(format!("User {id} not found")));
    }

    auth.delete_user(id).await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
```

`resolve_updated_password` is a free function in `src/auth/models.rs`, not
re-exported through `crate::auth::{...}` in `src/auth/mod.rs` — call it via
its full path (`crate::auth::models::resolve_updated_password`) rather than
adding it to the `use crate::auth::{...}` import list, since this plan
doesn't touch `src/auth/mod.rs`. `Request`/`Response`/`Json`/`AppError`/
`AppResult`/`User`/`get_auth` are all already imported in this file.

- [ ] **Step 6: Build to confirm it compiles**

Run: `cargo build`
Expected: builds clean.

- [ ] **Step 7: Wire the routes**

In `src/routes.rs`, find:

```rust
fn build_api_users_routes() -> Router {
    Router::with_path("users")
        .get(api::users::index)
        .post(api::users::create)
}
```

Replace with:

```rust
fn build_api_users_routes() -> Router {
    Router::with_path("users")
        .get(api::users::index)
        .post(api::users::create)
        .push(
            Router::with_path("{id}")
                .put(api::users::update)
                .delete(api::users::delete),
        )
}
```

- [ ] **Step 8: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. The two new handlers touch
`get_auth()` global state (same limitation as every other `src/api/`
handler in this codebase) — no isolated handler-level unit test;
correctness rests on the already-tested `resolve_updated_password` plus
this build/regression check.

- [ ] **Step 9: Manual smoke check**

With the dev server running (`cargo watch -x run` or `cargo run`), obtain an
admin JWT via `POST /api/users/login`, then exercise the new endpoints:

```bash
TOKEN="<jwt from login>"
# create a throwaway user to update/delete
curl -s -X POST http://localhost:5887/api/admin/users \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"username": "smoketest", "email": "smoketest@example.com", "password": "temp12345", "first_name": null, "last_name": null, "groups": []}'
# capture the returned "id", then:
curl -s -X PUT http://localhost:5887/api/admin/users/<id> \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"username": "smoketest2", "email": "smoketest2@example.com", "password": null, "first_name": null, "last_name": null, "groups": []}'
curl -s -X DELETE http://localhost:5887/api/admin/users/<id> -H "Authorization: Bearer $TOKEN"
curl -s -X PUT http://localhost:5887/api/admin/users/does-not-exist \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"username": "x", "email": "x@example.com", "password": null, "first_name": null, "last_name": null, "groups": []}'
```

Confirm: update with `password: null` keeps the user able to log in with
the original password; delete removes the user (confirm via
`GET /api/admin/users`); update/delete of a nonexistent id both return
`404`.

- [ ] **Step 10: Commit**

```bash
git add src/auth/models.rs src/auth/tests.rs src/api/users.rs src/routes.rs
git commit -m "feat(api): add update/delete to the Users JSON admin API"
```

---

### Task 2: Catalog API — `update_layer`

**Files:**
- Modify: `src/api/catalog.rs` (`UpdateLayerRequest`, `update_layer` handler)
- Modify: `src/routes.rs` (`build_api_catalog_routes`)

**Interfaces:**
- Consumes: `Category::from_id`, `Auth::resolve_groups_by_name`,
  `services::utils::normalize_name`,
  `Catalog::find_layer_by_id(&self, target_id: &str, state: StateLayer) -> Option<&Layer>`,
  `Catalog::update_layer(&mut self, layer: Layer) -> AppResult<()>`,
  `crate::invalidate_layer_tile_cache(layer_key: &str) -> AppResult<()>`
  (all existing, unchanged).
- Produces: `PUT /api/admin/catalog/layer/{id}`, renders the updated `Layer`
  as JSON.

- [ ] **Step 1: Add `UpdateLayerRequest` and the `update_layer` handler**

In `src/api/catalog.rs`, first extend the imports to bring in `StateLayer`.
Find:

```rust
use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth, get_catalog,
    models::{catalog::Layer, category::Category},
};
```

Replace with:

```rust
use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth, get_catalog,
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
    },
};
```

Then add, at the end of the file (after `create_layer`):

```rust
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateLayerRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
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
pub async fn update_layer(res: &mut Response, layer_form: UpdateLayerRequest) -> AppResult<()> {
    let category = Category::from_id(&layer_form.category).await?;

    let auth = get_auth().await.read().await;
    let groups: Vec<Group> = layer_form
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();
    drop(auth);

    let name = crate::services::utils::normalize_name(&layer_form.name)?;
    let layer_key = format!("{}_{}", category.name, name);

    let mut catalog = get_catalog().await.write().await;
    if catalog
        .find_layer_by_id(&layer_form.id, StateLayer::Any)
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "Layer {} not found",
            layer_form.id
        )));
    }

    let layer = Layer {
        id: layer_form.id,
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

    catalog.update_layer(layer.clone()).await?;
    drop(catalog);

    crate::invalidate_layer_tile_cache(&layer_key).await?;

    res.render(Json(&layer));
    Ok(())
}
```

Note: the layer's `name` is normalized once here (via `normalize_name`)
before both building the `layer_key` and being stored on the `Layer`
struct, instead of relying on `Catalog::update_layer`'s own internal
re-normalization (which the HTML handler leans on). This is safe —
`normalize_name` is idempotent, so `Catalog::update_layer`'s internal
re-normalization of an already-normalized name is a no-op — and it's
necessary here because `Catalog::update_layer` returns `AppResult<()>`, not
the updated `Layer`, so the handler needs its own already-correct clone to
render back to the caller.

- [ ] **Step 2: Build to confirm it compiles**

Run: `cargo build`
Expected: builds clean.

- [ ] **Step 3: Wire the route**

In `src/routes.rs`, find:

```rust
fn build_api_catalog_routes() -> Router {
    Router::with_path("catalog/layer")
        .get(api::catalog::list)
        .post(api::catalog::create_layer)
}
```

Replace with:

```rust
fn build_api_catalog_routes() -> Router {
    Router::with_path("catalog/layer")
        .get(api::catalog::list)
        .post(api::catalog::create_layer)
        .push(Router::with_path("{id}").put(api::catalog::update_layer))
}
```

- [ ] **Step 4: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. `update_layer` touches `get_auth()`/
`get_catalog()` global state (same limitation as `create_layer` in this same
file) — no isolated handler-level unit test; correctness rests on the
already-tested `Catalog`/`Category`/`Auth` methods it calls plus this
build/regression check.

- [ ] **Step 5: Manual smoke check**

```bash
TOKEN="<jwt from login>"
curl -s -X PUT http://localhost:5887/api/admin/catalog/layer/<existing-layer-id> \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"category": "<category id>", "database_id": "default", "geometry": "polygons", "name": "<layer name>", "alias": "Updated alias", "description": "", "schema": "public", "table": "<table>", "fields": [], "published": true, "groups": []}'
curl -s -X PUT http://localhost:5887/api/admin/catalog/layer/does-not-exist \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"category": "<category id>", "database_id": "default", "geometry": "polygons", "name": "x", "alias": "x", "description": "", "schema": "public", "table": "t", "fields": [], "published": true, "groups": []}'
```

Confirm: the update returns the updated layer with `alias` reflecting the
change; a request against a tile URL for that layer after the update
reflects any field changes (proves the `invalidate_layer_tile_cache` call
works through the API surface, not just the HTML one); the nonexistent-id
request returns `404`.

- [ ] **Step 6: Commit**

```bash
git add src/api/catalog.rs src/routes.rs
git commit -m "feat(api): add layer update to the Catalog JSON admin API"
```

---

### Task 3: Catalog API — `delete_layer`, `toggle_published`, `delete_layer_cache`

**Files:**
- Modify: `src/api/catalog.rs` (three new handlers)
- Modify: `src/routes.rs` (`build_api_catalog_routes`)

**Interfaces:**
- Consumes: `Catalog::find_layer_by_id`, `Catalog::delete_layer(&mut self, id: String) -> AppResult<()>`,
  `Catalog::swich_layer_published(&mut self, target_id: &str) -> AppResult<()>`,
  `get_cache_wrapper() -> &'static CacheWrapper`,
  `CacheWrapper::delete_layer_cache(&self, layer_name: &String) -> AppResult<()>`
  (all existing, unchanged).
- Produces: `DELETE /api/admin/catalog/layer/{id}`,
  `PATCH /api/admin/catalog/layer/{id}/publish`,
  `DELETE /api/admin/catalog/layer/{id}/cache`.

- [ ] **Step 1: Add `get_cache_wrapper` to the imports**

In `src/api/catalog.rs`, find (as left by Task 2):

```rust
use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth, get_catalog,
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
    },
};
```

Replace with:

```rust
use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth, get_cache_wrapper, get_catalog,
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
    },
};
```

- [ ] **Step 2: Add the three handlers**

Append to the end of `src/api/catalog.rs`:

```rust
#[handler]
pub async fn delete_layer(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let mut catalog = get_catalog().await.write().await;
    if catalog.find_layer_by_id(&id, StateLayer::Any).is_none() {
        return Err(AppError::NotFound(format!("Layer {id} not found")));
    }

    catalog.delete_layer(id).await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}

#[handler]
pub async fn toggle_published(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let mut catalog = get_catalog().await.write().await;
    if catalog.find_layer_by_id(&id, StateLayer::Any).is_none() {
        return Err(AppError::NotFound(format!("Layer {id} not found")));
    }

    catalog.swich_layer_published(&id).await?;

    let layer = catalog
        .find_layer_by_id(&id, StateLayer::Any)
        .ok_or_else(|| AppError::NotFound(format!("Layer {id} not found")))?;
    res.render(Json(layer));
    Ok(())
}

#[handler]
pub async fn delete_layer_cache(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let layer_name = {
        let catalog = get_catalog().await.read().await;
        let layer = catalog
            .find_layer_by_id(&id, StateLayer::Any)
            .ok_or_else(|| AppError::CacheNotFound(id.to_string()))?;
        format!("{}_{}", layer.category.name, layer.name)
    };

    get_cache_wrapper().delete_layer_cache(&layer_name).await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
```

`toggle_published` re-fetches the layer after `swich_layer_published`
(still under the same write lock — no re-acquire needed) so the response
reflects the new `published` value without requiring a follow-up `GET`.

- [ ] **Step 3: Build to confirm it compiles**

Run: `cargo build`
Expected: builds clean.

- [ ] **Step 4: Wire the routes**

In `src/routes.rs`, find (as left by Task 2):

```rust
fn build_api_catalog_routes() -> Router {
    Router::with_path("catalog/layer")
        .get(api::catalog::list)
        .post(api::catalog::create_layer)
        .push(Router::with_path("{id}").put(api::catalog::update_layer))
}
```

Replace with:

```rust
fn build_api_catalog_routes() -> Router {
    Router::with_path("catalog/layer")
        .get(api::catalog::list)
        .post(api::catalog::create_layer)
        .push(
            Router::with_path("{id}")
                .put(api::catalog::update_layer)
                .delete(api::catalog::delete_layer)
                .push(Router::with_path("publish").patch(api::catalog::toggle_published))
                .push(Router::with_path("cache").delete(api::catalog::delete_layer_cache)),
        )
}
```

- [ ] **Step 5: Build and run the full suite**

Run: `cargo build && cargo test`
Expected: builds clean, all tests pass. Same rationale as Task 2 Step 4 —
these three handlers touch `get_catalog()` global state; no isolated
handler-level unit test; correctness rests on the already-tested
`Catalog`/`CacheWrapper` methods they call plus this build/regression check.

- [ ] **Step 6: Manual smoke check**

```bash
TOKEN="<jwt from login>"
curl -s -X PATCH http://localhost:5887/api/admin/catalog/layer/<layer-id>/publish \
  -H "Authorization: Bearer $TOKEN"
curl -s -X DELETE http://localhost:5887/api/admin/catalog/layer/<layer-id>/cache \
  -H "Authorization: Bearer $TOKEN"
curl -s -X DELETE http://localhost:5887/api/admin/catalog/layer/<layer-id> \
  -H "Authorization: Bearer $TOKEN"
curl -s -X DELETE http://localhost:5887/api/admin/catalog/layer/does-not-exist \
  -H "Authorization: Bearer $TOKEN"
curl -s -X PATCH http://localhost:5887/api/admin/catalog/layer/does-not-exist/publish \
  -H "Authorization: Bearer $TOKEN"
curl -s -X DELETE http://localhost:5887/api/admin/catalog/layer/does-not-exist/cache \
  -H "Authorization: Bearer $TOKEN"
```

Confirm: `publish` flips the layer's `published` field and returns it in the
response body; `cache` clears the tile cache (a subsequent tile request
regenerates rather than serving a stale cached tile); `delete` removes the
layer (confirm via `GET /api/catalog/layer`); all three return `404` for a
nonexistent id (use a real layer id you don't mind deleting for the delete
step, since it's destructive — create a throwaway layer via
`POST /api/admin/catalog/layer` first if none is expendable).

- [ ] **Step 7: Commit**

```bash
git add src/api/catalog.rs src/routes.rs
git commit -m "feat(api): add delete/publish-toggle/cache-clear to the Catalog JSON admin API"
```

---

## Final verification

- [ ] Run `cargo test` one more time end-to-end — full green.
- [ ] Run `cargo clippy --all-targets` — no new warnings introduced by this
  plan's files (`src/auth/models.rs`, `src/auth/tests.rs`,
  `src/api/users.rs`, `src/api/catalog.rs`, `src/routes.rs`).
- [ ] Re-read the design spec
  (`docs/superpowers/specs/2026-07-29-admin-api-completion-phase3-design.md`)
  against the 3 tasks above and confirm each design section is covered:
  Users update/delete → Task 1, Catalog update → Task 2, Catalog
  delete/publish/cache → Task 3.
- [ ] Confirm no file outside this plan's Global Constraints list was
  touched.
