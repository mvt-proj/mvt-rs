# Admin API Completion — Phase 3: Users update/delete, Catalog update/delete/publish/cache-delete — Design

**Date:** 2026-07-29
**Status:** Approved by user
**Scope:** Third and final phase to bring the JSON REST admin API
(`src/api/`) to parity with the HTML admin panel (`src/html/admin/`). Phase 1
(bugs + shared infra) shipped 2026-07-27. Phase 2 (Groups/Categories/Styles
CRUD + delete guards + Style parity fixes) shipped 2026-07-28. This phase
closes the two remaining partial surfaces: `api::users` (only
login/list/create) and `api::catalog` (only list/create).

## Problem

`src/api/users.rs` has no update or delete handler — only `login`, `index`
(list), and `create`. `src/api/catalog.rs` has no update, delete, publish
toggle, or cache-clear handler — only `list` and `create_layer`. The HTML
admin panel (`src/html/admin/users.rs`, `src/html/admin/catalog.rs`) already
has all of these; the API was simply never finished for these two entities
(pre-existing gap, tracked since the Phase 1/2 audit).

Both entities differ from Phase 2's Groups/Categories/Styles in one
structural way: their model-layer mutation methods
(`Auth::update_user`/`delete_user`, `Catalog::update_layer`/`delete_layer`/
`swich_layer_published`) do **not** error on a missing id — they silently
no-op (`warn!`/`println!` and return `Ok(())`), unlike `Group`/`Category`/
`Style`'s `from_id() -> AppResult<Self>` + instance-method pattern from
Phase 2. This is existing, HTML-facing behavior and is out of scope to
change here (see Decisions).

## Decisions (locked during brainstorming, 2026-07-29)

1. **Charter**: strict CRUD completion for Users and Catalog/Layer only. The
   four items deferred from Phases 1 and 2 (the `GET /api/catalog/layer`
   auth gap, the `resolve_groups_by_name` DRY sweep, duplicate-name creates
   returning 500 instead of 409, missing `UNIQUE(category, name)` on
   `styles`) are deferred again — not bundled into this phase.
2. **Delete guard**: none. No self-delete or last-admin protection for user
   deletion — exact parity with the HTML admin's current behavior, which has
   no such guard either. Not introducing new safety behavior beyond what was
   asked.
3. **Password update semantics**: `password: Option<String>` in the update
   DTO. `None` (field omitted/null) means keep the existing hash; `Some(pw)`
   means set a new password. Chosen over HTML's empty-string-means-keep
   convention because it's the idiomatic JSON-API representation of "field
   not being changed."
4. **Catalog action routing**: publish-toggle and cache-clear are each their
   own sub-route/handler (mirroring the HTML admin's separate actions), not
   folded into the general update endpoint.
5. **Existence checks (404)**: added as pre-checks in the new API handlers
   only. The underlying model methods (`Auth::update_user`, `Auth::delete_user`,
   `Catalog::update_layer`, `Catalog::delete_layer`,
   `Catalog::swich_layer_published`) are **not** modified — their current
   silent-no-op-on-missing-id behavior keeps serving the HTML admin
   unchanged. Each new API handler independently checks existence
   (`get_user_by_id` / `find_layer_by_id`) before calling the mutator, and
   returns `AppError::NotFound` if the id doesn't resolve.
6. **Cache invalidation on layer delete**: not added. The HTML
   `delete_layer` handler does not invalidate the tile cache today; the new
   API `delete_layer` handler matches that (pre-existing gap, not
   introduced or fixed by this phase).

## Design

### New routes

Two router-factory functions extended, following the `{id}`-sub-router
pattern Phase 2 established for groups/categories/styles, mounted inside the
existing `admin` block (behind `jwt_auth_handler` → `validate_token` →
`require_api_admin`, unchanged):

```
PUT    /api/admin/users/{id}
DELETE /api/admin/users/{id}

PUT    /api/admin/catalog/layer/{id}
DELETE /api/admin/catalog/layer/{id}
PATCH  /api/admin/catalog/layer/{id}/publish
DELETE /api/admin/catalog/layer/{id}/cache
```

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

### Users: `update` / `delete`

`UpdateUserRequest` in `src/api/users.rs`, following the `UpdateStyleRequest`
id-from-path convention:

```rust
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateUserRequest<'a> {
    #[salvo(extract(source(from = "param")))]
    id: String,
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: Option<String>,
    groups: Option<Vec<String>>,
}
```

`update` handler: acquire `get_auth().write()`, look up the target user via
`get_user_by_id(&id)` — `AppError::NotFound` if absent. Resolve password:
`Some(pw)` → `auth.get_encrypt_psw(pw)`; `None` → reuse the existing user's
`password` field. Resolve `groups` via the existing
`auth.resolve_groups_by_name` (Phase 1 helper, already used by `create`).
Build the updated `User` and call `auth.update_user(user)`.

`delete` handler: acquire `get_auth().write()`, check `get_user_by_id(&id)`
— `AppError::NotFound` if absent — then `auth.delete_user(id)`.

### Catalog: `update_layer` / `delete_layer` / `toggle_published` / `delete_layer_cache`

`UpdateLayerRequest` in `src/api/catalog.rs`: identical fields to the
existing `NewLayerRequest`, minus `category`/body-level id — `id` comes from
the path via `#[salvo(extract(source(from = "param")))]`, same as
`UpdateStyleRequest`.

`update_layer` handler: resolve `Category::from_id(&layer_form.category)`
(already 404-safe), resolve `groups` via `auth.resolve_groups_by_name`,
acquire `get_catalog().write()`, check `find_layer_by_id(&id, StateLayer::Any)`
— `AppError::NotFound` if absent — build the `Layer`, call
`catalog.update_layer(layer)`, then call
`crate::invalidate_layer_tile_cache(&layer_key)` on success (same as the
HTML handler, using the same `{category_name}_{normalized_layer_name}` key
format).

`delete_layer` handler: acquire `get_catalog().write()`, check
`find_layer_by_id` — 404 if absent — then `catalog.delete_layer(id)`. No
cache invalidation (Decision 6).

`toggle_published` handler: acquire `get_catalog().write()`, check
`find_layer_by_id` — 404 if absent — then `catalog.swich_layer_published(&id)`,
then re-fetch and render the updated `Layer` as JSON — same "return the
updated resource" convention `update_layer` and Phase 2's group/category/
style updates use, so callers don't need a follow-up `GET` to see the new
`published` value.

`delete_layer_cache` handler: same logic as
`html::admin::catalog::delete_layer_cache` — look up the layer, build the
`{category_name}_{layer_name}` cache key, return `AppError::CacheNotFound`
if the layer doesn't exist, else `get_cache_wrapper().delete_layer_cache(&key)`.

Response shapes: `update_layer` and `toggle_published` render the updated
`Layer` as JSON. `delete_layer` and `delete_layer_cache` render
`{"deleted": true}` (matching Phase 2's `delete` handlers in
`api/{groups,categories,styles}.rs`). All via `AppResult<Json<T>>`.

### Testing

- `src/auth/tests.rs` / relevant handler tests: cover `update`/`delete`
  happy paths and the 404-on-missing-id case, following Phase 2's
  `require_api_admin` `TestClient` pattern where a global-state test is
  unavoidable (both `Auth::update_user`/`delete_user` and
  `Catalog::update_layer`/`delete_layer`/`swich_layer_published` require the
  `OnceLock` globals — no new pure-function extraction opportunity exists
  here the way `count_group_references` did in Phase 2, since these
  operations don't need a pre-delete scan).
- Password-omitted-keeps-hash and password-provided-changes-hash are the two
  cases worth a dedicated assertion for `update`.
- Verification gate per task: `cargo build && cargo test`.

## Out of scope (tracked separately, not part of this spec)

- `GET /api/catalog/layer` auth gap (deferred a third time).
- `resolve_groups_by_name` DRY sweep in `html/admin/{catalog,users}.rs`.
- Duplicate-name creates returning 500 instead of 409 (groups/categories).
- Missing `UNIQUE(category, name)` constraint on `styles`.
- Partial (`PATCH`-style) field updates for Users/Catalog (full replacement
  only, matching Phase 2's convention).
- Cache invalidation on layer delete (pre-existing HTML gap, not fixed here).
- Self-delete / last-admin delete guard for Users (explicitly declined).
