# Admin API Completion — Phase 2: Groups/Categories/Styles CRUD — Design

**Date:** 2026-07-28
**Status:** Approved by user
**Scope:** Second of three phases to bring the JSON REST admin API
(`src/api/`) to parity with the HTML admin panel (`src/html/admin/`). Phase 1
(bugs + shared infra: password-hash leak fix, `Auth::resolve_groups_by_name`,
JWT `groups` claim + `require_api_admin` gate, `AppResult` standardization in
`database.rs`, shared `invalidate_layer_tile_cache`) shipped and merged to
`main` 2026-07-27. This phase adds the Groups/Categories/Styles CRUD
endpoints that were entirely missing, plus two small parity fixes to `Style`
uncovered while researching this phase. Phase 3 (Users update/delete,
Catalog update/delete/publish/cache-delete) is a separate spec, planned
after this phase ships.

## Problem

A research pass (2026-07-28, via an Explore agent covering
`src/html/admin/{groups,categories,styles}.rs`, `src/api/`, `src/config/`,
`src/models/`, and `src/routes.rs`) confirmed and expanded on Phase 1's
audit:

- `src/api/` has no `groups.rs`, `categories.rs`, or `styles.rs` at all — no
  routes, no handlers. Fully greenfield.
- There is no delete-time "in use" guard anywhere in the codebase for
  `Group` or `Category`. The SQLite schema has no foreign-key constraints
  (`layers.category`, `layers.groups`, `styles.category` are plain
  unconstrained `TEXT`), and `Layer`/`Style` embed full denormalized
  `Group`/`Category` snapshots rather than ids. Deleting a `Group`/`Category`
  still referenced by a `Layer`/`Style`/`User` doesn't error — it silently
  orphans the reference, and can later make `get_layers`/`get_styles`
  **crash** (`row.get::<String, _>("category_id")` fails to decode a SQL
  `NULL` produced by the dangling join). Adding new `DELETE` endpoints
  without addressing this would just add more callers hitting the same
  crash.
- `Style` never got the two parity fixes Phase 1 gave `Category`:
  - `Style::from_id` (and `get_style`) let `sqlx::Error::RowNotFound` bubble
    up as `AppError::SQLError` (500), instead of being mapped to
    `AppError::NotFound` (404) the way `Category::from_id` is.
  - `Style`'s model methods (`update_style`, `delete_style`) don't touch the
    in-memory `STYLES` cache used by public style/legend endpoints — every
    HTML handler has to remember to call `crate::reload_styles_cache()`
    manually afterward (`html/admin/styles.rs:125,172,200`). A new API
    handler would inherit the same footgun.
- Two further issues were found but are explicitly **out of scope** for this
  phase (see "Out of scope" below): `GET /api/catalog/layer` has no auth and
  returns unpublished/group-restricted layers unfiltered; four call sites in
  `html/admin/catalog.rs` and `html/admin/users.rs` still hand-roll group
  resolution instead of using Phase 1's `Auth::resolve_groups_by_name`.

## Decisions (locked during brainstorming, 2026-07-28)

1. **Charter**: strict CRUD for Groups/Categories/Styles, plus the two
   `Style` parity fixes above (in scope because this phase already touches
   `Style` end-to-end). The auth gap on `GET /api/catalog/layer` and the DRY
   sweep are deferred to their own follow-ups, not bundled here.
2. **Delete guard**: new `DELETE` endpoints for `Group`/`Category` must
   block deletion when the entity is still referenced, returning a 409
   instead of silently orphaning. `Style` needs no guard (nothing
   references a `Style`).
3. **Guard location**: implemented once, in the model layer
   (`Group::delete_group`, `Category::delete_category`), not duplicated per
   HTML/API handler. Both surfaces inherit it automatically — this also
   fixes the same silent-orphan risk that exists today in
   `html/admin/{groups,categories}.rs::delete_*`, as a side effect of
   fixing it in one place.
4. **Guard implementation**: in-memory scan of the existing global caches
   (`get_catalog().read().await.layers`, `get_auth().read().await.users`,
   the `STYLES` cache) rather than a fresh SQL query. These caches are
   already the source of truth for reads elsewhere in the app (e.g.
   `Category::update_category`'s existing cascade into `catalog.layers`
   uses the same pattern) — no known drift scenario exists since every
   mutation path already goes through the models that keep these caches in
   sync.
5. **Update semantics**: full replacement only (matches the existing HTML
   forms and the `create_layer`/`create` DTO pattern from Phase 1) — no
   partial/`PATCH`-style updates this phase.
6. **Style `category` reference**: by id (matches the existing HTML style
   form and `Category::from_id`), not by name — consistent with how
   `create_layer` already resolves its category reference.
7. **Response/error pattern**: `AppResult<Json<T>>` return type directly
   (the `api/database.rs` idiom from Phase 1), no manual status-code
   plumbing.

## Design

### New routes

Three new router-factory functions, following the existing
`build_api_users_routes()` / `build_api_database_routes()` /
`build_api_catalog_routes()` pattern in `src/routes.rs`, all mounted inside
the existing `admin` block (behind `jwt_auth_handler` → `validate_token` →
`require_api_admin`, unchanged from Phase 1):

```
GET    /api/admin/groups
POST   /api/admin/groups
PUT    /api/admin/groups/{id}
DELETE /api/admin/groups/{id}

GET    /api/admin/categories
POST   /api/admin/categories
PUT    /api/admin/categories/{id}
DELETE /api/admin/categories/{id}

GET    /api/admin/styles
POST   /api/admin/styles
PUT    /api/admin/styles/{id}
DELETE /api/admin/styles/{id}
```

New files: `src/api/groups.rs`, `src/api/categories.rs`, `src/api/styles.rs`,
each registered in `src/api/mod.rs`.

### Request/response shapes

- **Groups** — create/update body: `{ name: String, description: String }`.
  Maps directly to `Group::new` / `group.update_group`. List/get responses
  serialize `Group` as-is (no sensitive fields).
- **Categories** — create/update body: `{ name: String, description: String }`.
  Maps to `Category::new` / `category.update_category` (the existing
  update-time cascade into `catalog.layers` is unchanged).
- **Styles** — create/update body:
  `{ name: String, category: String /* id */, description: String, style: String /* raw JSON/MapLibre style text */ }`.
  `category` resolved via `Category::from_id` (already 404-safe from Phase
  1). `style` validated via the existing
  `services::utils::validate_style_json` (→ `AppError::InvalidInput` on
  failure). Maps to `Style::new` / `style.update_style`.

All three follow the id-path-param convention (`PUT|DELETE /{id}`) already
implicit in the HTML admin's edit/delete routes.

### Delete guard

New `AppError::Conflict(String)` variant in `src/error.rs`, mapped to
`StatusCode::CONFLICT` (409), added alongside the existing `Forbidden(String)`
variant from Phase 1.

`Group::delete_group` and `Category::delete_category` gain a pre-delete
check:

- `Group::delete_group`: scans `get_auth().read().await.users` for any
  `User.groups` entry matching this group's id, and
  `get_catalog().read().await.layers` for any `Layer.groups` entry matching
  it. If either is non-empty, returns
  `AppError::Conflict(format!("Group '{name}' is in use by {n} user(s) and {m} layer(s)"))`
  instead of deleting.
- `Category::delete_category`: scans `get_catalog().read().await.layers`
  for `Layer.category.id` matches and the `STYLES` cache for
  `Style.category.id` matches, with the same Conflict-or-proceed logic.

Both HTML delete handlers (`html/admin/groups.rs::delete_group`,
`html/admin/categories.rs::delete_category`) call these same model methods
already and need no signature changes — they inherit the guard for free
(and `AppError::Conflict` renders as an HTML error page via the existing
`AppError::write` content negotiation, same as any other `AppError`).

### Style parity fixes

- `Style::from_id` (and, if needed, `get_style` in `config/styles.rs`): map
  `sqlx::Error::RowNotFound` → `AppError::NotFound`, mirroring
  `Category::from_id`.
- `style.update_style` and `style.delete_style`: call
  `crate::reload_styles_cache()` internally, at the end of each method,
  instead of leaving it to the caller. The three existing HTML call sites
  that currently call `reload_styles_cache()` manually after these methods
  (`html/admin/styles.rs:125,172,200`) drop their now-redundant call.

### Testing

- `src/config/{groups,categories,styles}.rs` already have unit tests with an
  in-memory SQLite pool (asserting `bump_config_version` is called on
  mutation) — extend with guard tests (in-use blocks with `Conflict`,
  not-in-use proceeds) wherever testable without the global `OnceLock`
  state; where a test would need `get_catalog()`/`get_auth()` (as
  `Category::from_id`'s 404 test did in Phase 1), document it as a
  regression guard rather than a failing-first TDD test.
- New HTTP handlers: `salvo::test::TestClient` integration tests following
  the `require_api_admin` pattern from Phase 1
  (`src/auth/handlers.rs` `#[cfg(test)] mod tests`), covering per entity:
  create/list/update/delete happy paths, delete blocked by guard (409 for
  Group/Category), update/delete of a nonexistent id (404), and (Styles
  only) invalid style JSON (400).
- Verification gate per task: `cargo build && cargo test` (no release
  builds), matching Phase 1's SDD execution.

## Out of scope (tracked separately, not part of this spec)

- `GET /api/catalog/layer` has no auth hoop and returns all layers
  unfiltered (unpublished + group-ACL'd), unlike `services/tilejson.rs`
  which uses `get_published_layers()` + `validate_user_groups()`. Pre-
  existing, not introduced by Phase 1 or 2.
- DRY sweep: `html/admin/catalog.rs:152-161,220-229` and
  `html/admin/users.rs:120-124,177-181` still hand-roll group-name
  resolution instead of `Auth::resolve_groups_by_name`.
- Partial (`PATCH`-style) updates.
- Phase 3: Users update/delete, Catalog update/delete/publish/cache-delete.
