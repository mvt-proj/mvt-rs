# MapLibre Style Schema Validation — Design

**Date:** 2026-07-12
**Status:** Approved by user
**Scope:** Level 1 of the style-editor roadmap: validate styles against the
MapLibre Style Spec in the admin editor. A graphical guided editor (level 2)
is explicitly out of scope for this spec.

## Problem

The admin style editor (`templates/admin/styles/{new,edit}.html`) is a raw
JSONEditor widget. It only checks JSON syntax; a style can violate the
MapLibre Style Spec (wrong property names, bad types, invalid expressions)
and nothing warns the user until the map silently breaks. Additionally, the
server never validates the `style` field: a syntactically broken JSON can be
stored, and `Style::to_json()` silently degrades it to `{}`.

## Context

Two style shapes coexist in the `styles` SQLite table:

1. **Full map styles** — have `"version": 8`, `sources`, `layers`
   (e.g. `mapa1`). Detected by `Style::is_map()`.
2. **Layer fragments** — `{"layers": [...]}` with no `version`/`sources`
   (e.g. `politico`, `zonas`). Layers reference a `source` name that the
   consuming client injects; some layers omit `source` entirely (valid in
   the mvt-rs fragment convention, invalid per the strict spec).

There is no Rust implementation of the MapLibre style validator. The
official validator is `@maplibre/maplibre-gl-style-spec` (npm), the same
one Maputnik uses. Its `dist/index.mjs` bundle (v25.0.2, ~290 KB) is
self-contained (no bare imports) and exports `validateStyleMin`, so it can
be imported directly from a CDN with `<script type="module">`. The admin
panel already loads JSONEditor and maplibre-gl from CDNs, so a pinned CDN
import matches the existing pattern.

## Decisions

- **Validate in the browser** with the official package, pinned:
  `https://cdn.jsdelivr.net/npm/@maplibre/maplibre-gl-style-spec@25.0.2/dist/index.mjs`.
- **Spec errors warn but never block** saving or applying. Only
  syntactically invalid JSON keeps blocking submission (current behavior).
- **Server-side defense in depth only**: `create_style` / `update_style`
  handlers reject non-parseable JSON with a 400. No spec validation in Rust.

## Components

### 1. `static/js/style-validator.js` (new, ES module)

Shared by `new.html` and `edit.html`. Exports:

- `validateStyle(json) -> [{ path, message }]`
  - If `json.version` exists → run `validateStyleMin(json)` directly.
  - Otherwise (fragment) → build a synthetic style before validating:
    - inject `version: 8` and a dummy `glyphs` URL (avoids the false
      positive "use of text-field requires glyphs");
    - collect every `layer.source` name and create a dummy
      `{"type": "vector"}` source for each;
    - for layers with no `source`, inject a dummy source name (fragment
      convention allows omitting it);
    - validate the synthetic style, drop errors that point at synthetic
      parts (root `version`/`glyphs`/`sources`, injected `source`
      properties), and re-map remaining error paths so they reference the
      user's original JSON (e.g. `layers[2].paint.fill-color`).
- Error objects from `validateStyleMin` carry `message` strings that
  already include the path; the module normalizes them into
  `{ path, message }` for display.

### 2. Editor integration (`new.html`, `edit.html`)

- Import the module with `<script type="module">`.
- Run validation on page load and on JSONEditor's `onChangeText`,
  debounced ~400 ms. While the buffer does not parse as JSON, the lint
  panel is cleared/hidden — syntax feedback stays the job of the existing
  `#jsonError` element.
- New panel below the editor (`#styleLintPanel`):
  - valid → green check + "style is valid" message;
  - errors → list of `path: message` rows.
- Panel title / valid-message strings go through Fluent i18n (new keys in
  the locale files, ES + EN). Validator messages remain in English as
  produced by the package.
- Save/apply flow unchanged: `beforeHtmxPost` / submit handler keep
  blocking only on unparseable JSON.

### 3. Server-side JSON check

In `src/html/admin/styles.rs`, `create_style` and `update_style` parse the
submitted `style` string with `serde_json::from_str::<serde_json::Value>`
and return `AppError::BadRequest` (400) when it fails, before persisting.

## Error handling

- Validator module fails to load (CDN down): validation silently degrades —
  panel hidden, editor fully usable. A `console.warn` is logged. Saving is
  unaffected.
- Fragment wrapping throws (unexpected shape): treat as "no spec errors"
  rather than crashing the editor; log to console.

## Testing

- **Rust unit tests**: `create_style`/`update_style` reject invalid JSON
  with 400; valid JSON still persists.
- **Manual browser verification** (no JS test infra in repo):
  - full style `mapa1` → panel shows valid;
  - fragment `politico` → panel shows valid (no false positives from the
    synthetic wrapper);
  - induced error (`"fill-color": 42`) → panel lists the error with the
    correct path;
  - spec errors do not prevent Apply/Update;
  - broken JSON still blocks submission.

## Out of scope

- Graphical/guided style editing (level 2 — separate spec).
- Rust-side spec validation.
- Validating styles at read/serve time (`/services/styles/...`).
- Vendoring the validator bundle (CDN keeps parity with existing assets;
  revisit only if offline deployments become a requirement).
