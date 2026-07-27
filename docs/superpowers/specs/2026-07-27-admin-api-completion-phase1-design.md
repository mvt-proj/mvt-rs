# Admin API Completion — Phase 1: Bugs + Shared Infra — Design

**Date:** 2026-07-27
**Status:** Approved by user
**Scope:** First of three phases to bring the JSON REST admin API
(`src/api/`) to parity with the HTML admin panel (`src/html/admin/`). This
phase covers security/correctness bugs and cross-cutting infrastructure that
every later endpoint depends on. Phase 2 (simple CRUD: Groups, Categories,
Styles) and Phase 3 (complex CRUD: Users update/delete, Catalog
update/delete/publish/cache-delete) are separate specs, planned after this
phase ships.

## Problem

The API was started alongside the HTML admin panel but never kept current.
An audit (2026-07-27) found:

- Groups/Categories/Styles have no API CRUD at all; Users and Catalog are
  partial (list/create only, no update/delete).
- `api::users::create` accepts a `groups` field but discards it
  (hardcodes `Vec::new()`).
- `GET /api/admin/users` serializes the full `User`, including the Argon2
  password hash — `auth::models::User` has no `#[serde(skip_serializing)]`
  on `password`.
- `api::catalog::create_layer` parses a raw `Layer` JSON body directly,
  requiring the client to send full nested `Category`/`Group` objects —
  inconsistent with the id/name-reference pattern the HTML admin already
  uses.
- No admin-role check anywhere in the API: `JwtClaims` carries no
  groups/role claim, so any valid JWT can hit user-management and layer
  endpoints. The HTML admin gates equivalent pages via `require_user_admin`
  (session-based); the API has no JWT-based equivalent.
- `src/api/database.rs` uses `Result<Json<T>, StatusError>` with a
  hand-rolled match/log block per handler, instead of the project-wide
  `AppResult<T>` / `AppError` standard (per `CLAUDE.md`).
- Tile-cache invalidation (with cluster deferred-delay support) lives only
  inside `html/admin/catalog.rs::update_layer` — not reusable from the API
  handlers Phase 3 will add.

## Decisions (locked during brainstorming)

1. **Scope**: API-to-HTML parity only. `DbRegistry` (Postgres connection
   management) is out of scope on both surfaces — sysadmin territory
   (config.yaml/env vars). Monitor JSON endpoint excluded for now.
2. **Request shape**: layer/style/user endpoints take simple references
   (category by id, groups by name list) resolved server-side — matching
   the existing HTML admin pattern (`Category::from_id`,
   `Auth::find_group_by_name`). Never nested full `Category`/`Group`
   objects.
3. **Response/error pattern**: all of `src/api/` standardizes on
   `AppResult<T>` / `AppError`.
4. **Cache/reload logic**: shared functions callable from both HTML and API
   handlers, not duplicated per surface.
5. **Ordering**: bugs + shared infra first (this spec) → simple CRUD
   (Groups/Categories/Styles) → complex CRUD (Users update/delete, Catalog
   update/delete/publish/cache-delete).
6. **JWT admin claim shape**: `groups: Vec<String>` (full group-name list),
   mirroring `User::is_admin()`'s existing
   `groups_as_vec_string().contains("admin")` check — not a precomputed
   `is_admin: bool`. Chosen for flexibility: future endpoints can gate on
   groups other than `"admin"` without re-touching the claim shape or
   forcing re-login.

## Components

### 1. Security fix — hide password hash in API responses

`src/auth/models.rs`: add `#[serde(skip_serializing)]` to `User.password`.

Verified safe: `password` is never populated via `Deserialize` on `User`
itself. It's always set via direct struct-literal construction — from raw
`sqlx::Row::get("password")` in `config/users.rs::get_users`, or from a
hashed value assigned in the HTML/API create/update handlers (which take
plaintext password through separate DTOs, hash it, then build the `User`
literal). Basic-auth password comparison (`validate_user`/`validate_psw`)
reads `user.password` via direct field access, never through serde. No
handler anywhere does `parse_json::<User>()` or extracts `User` as a
request body. So `skip_serializing` only removes the field from outgoing
`Json(&user)` responses; nothing else changes.

### 2. Bug fix — `api::users::create` groups

`src/api/users.rs`: change `NewUser.groups` from `Vec<Option<Group>>`
(nested full objects) to `groups: Option<Vec<String>>` (names), resolved
against `auth.find_group_by_name` the same way
`html/admin/catalog.rs::create_layer` resolves layer groups: unmatched
names are silently dropped (`filter_map`), not rejected — this matches the
existing HTML behavior exactly, so API and HTML stay consistent rather than
introducing a stricter validation rule only the API enforces. Replaces the
current hardcoded `Vec::new()`.

### 3. Fix — `api::catalog::create_layer` request shape

`src/api/catalog.rs`: replace `req.parse_json::<Layer>()` with a new
`NewLayerRequest` DTO shaped like `html/admin/catalog.rs`'s `NewLayer`
(`category` as an id string, `groups` as a list of names), resolved
server-side with `Category::from_id` + `auth.find_group_by_name`. Brings
the one existing write endpoint in line with decision #2 before Phase 3
adds `update`.

### 4. Admin-role JWT claim + enforcement

- `src/auth/models.rs::JwtClaims`: add `groups: Vec<String>`, populated in
  `Auth::login()` from `user.groups_as_vec_string()`.
- `src/error.rs`: add `AppError::Forbidden(String)` →
  `StatusCode::FORBIDDEN` (403). Distinct from the existing
  `UnauthorizedAccess`/`InvalidCredentials` (401, no/invalid token) —
  `Forbidden` means "valid token, wrong role."
- `src/auth/handlers.rs`: new `require_api_admin` handler, analogous to
  `require_user_admin` but reading `depot.jwt_auth_data::<JwtClaims>()`
  instead of the session. Returns `AppError::Forbidden` when `"admin"` is
  not in `claims.groups`.
- `src/routes.rs`: add `.hoop(auth::require_api_admin)` to the `admin`
  sub-router inside `build_api_routes()`, so it covers
  `build_api_users_routes()`, `build_api_database_routes()`,
  `build_api_catalog_routes()`, and every endpoint Phase 2/3 add under the
  same sub-router automatically.

### 5. `AppResult` standardization in `database.rs`

`src/api/database.rs`: change `schemas`, `tables`, `fields`, `srid` from
`Result<Json<T>, StatusError>` (with a repeated manual match/log/wrap block
per handler) to `AppResult<Json<T>>`. Since `query_schemas` /
`query_tables` / `query_fields` / `query_srid` already return `AppResult`,
each handler body collapses to `Ok(Json(query_x(...).await?))`.

### 6. Shared tile-cache invalidation

Extract the cache-invalidation block currently inline in
`html/admin/catalog.rs::update_layer` (delay-aware: immediate delete, or a
deferred `tokio::spawn` when `get_cache_invalidation_delay()` is `Some`,
for clustered owner/shared modes) into:

```rust
// src/main.rs, alongside reload_styles_cache() / get_cache_invalidation_delay()
pub async fn invalidate_layer_tile_cache(layer_key: &str) -> AppResult<()>
```

`html/admin/catalog.rs::update_layer` calls this instead of inlining the
logic (no behavior change). Phase 3's `api::catalog::update` will call the
same function.

## Data flow (admin API request, post-Phase-1)

```
Request → JwtAuth (validate_token, decodes JwtClaims) →
  require_api_admin (checks "admin" ∈ claims.groups, else 403) →
  handler (AppResult<T>) →
  AppError::write (Accept header decides HTML vs JSON error body)
```

## Error handling

- All new/changed handlers return `AppResult<T>`; errors flow through the
  existing `AppError` → `Writer` impl (HTML or JSON depending on `Accept`).
- `require_api_admin` short-circuits with 403 before any handler body runs.
- No behavior change for existing 401 paths (`validate_token` /
  `JwtAuthState::Unauthorized`/`Forbidden` from salvo's own token-structure
  checks) — `AppError::Forbidden` is additive, for the new role check only.

## Testing

Unit tests colocated per the project's `src/*/tests.rs` convention:

- `User` serializes without a `password` field (`serde_json::to_value`
  round-trip check); deserialization/DB-load paths unaffected (regression
  guard for decision above).
- `api::users::create` resolves valid group names to `Group`s and silently
  drops unknown group names (matches HTML behavior, no error).
- `api::catalog::create_layer` resolves `category` id and `groups` names
  correctly; 404 on unknown category id.
- `require_api_admin` passes for a token with `"admin"` in `groups`, 403s
  otherwise, 403s (not panics) when the claim is missing/malformed.
- `database.rs` handlers: existing behavior preserved for the success path;
  a forced DB error now surfaces as a proper `AppError` (500) instead of
  the old hand-rolled 400 — call out as an intentional status-code change
  in the implementation plan.

## Out of scope (this phase)

- Groups/Categories/Styles CRUD API (Phase 2).
- Users update/delete, Catalog update/delete/publish/delete-cache API
  (Phase 3).
- `DbRegistry` management API/UI (excluded entirely, decision #1).
- Monitor JSON endpoint changes.
- Re-authentication/token-refresh flow for users whose group membership
  changes after a token was issued (the `groups` claim is a snapshot at
  login time, same staleness window that already exists implicitly via
  session-based `require_user_admin` today — not a new problem introduced
  by this phase, not solved by it either).
