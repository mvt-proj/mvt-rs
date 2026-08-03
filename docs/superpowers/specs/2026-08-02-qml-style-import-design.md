# QML Style Import — Design

**Date:** 2026-08-02
**Status:** Approved by user
**Scope:** Let the admin style editor (`new.html` / `edit.html`) convert a
QGIS `.qml` style file into MapLibre layer JSON and prefill the JSONEditor,
using the `qml2maplibre` crate. Purely additive — the existing raw-JSON
editing workflow (`btnFullStyle`/`btnPartialStyle`) is untouched.

## Problem

Users who already have QGIS `.qml` styles for their layers have no way to
reuse them in mvt-rs — they have to hand-write MapLibre style JSON from
scratch. `qml2maplibre` (in the sibling workspace
`/home/jose/trabajos/mvt-proj/any2maplibre`) already converts QML XML into
MapLibre layer JSON; this spec wires it into the admin style form.

## Context

- `qml2maplibre::convert(qml_xml: &str, source_layer: &str, mode: OutputMode) -> Result<ConversionResult, ConvertError>`
  where `ConversionResult { layers: Vec<MapLibreLayer>, warnings: Vec<String> }`.
  `MapLibreLayer` derives `Serialize`. `OutputMode` has variants `Compact`
  and `QgisCompatible`. `convert()` does not set a layer's `source`; callers
  add that afterward if needed (mvt-rs's fragment convention already omits
  `source` on imported/partial layers — see below — so this is not needed
  here).
- `ConvertError` variants: `Xml(quick_xml::Error)`, `NoRules`,
  `AllRulesUnsupported(Vec<String>)`, `UnexpectedEof`.
- The style editor (`templates/admin/styles/{new,edit}.html`) is a
  JSONEditor bound to `<input type="hidden" id="jsonInput" name="style">`.
  Two existing buttons, `btnFullStyle`/`btnPartialStyle`, call
  `editor.set(...)` to prefill example JSON — no server round trip. A style
  can be a full map style (`version`/`sources`/`layers`) or a **fragment**
  (`{"layers": [...]}` only, source injected by the consuming client) — see
  [[maplibre-style-validation]]. QML-imported layers follow the fragment
  convention: no `source` key.
- `static/js/style-validator.js` listens for `style-editor-changed` /
  `style-editor-ready` events and renders live MapLibre-spec validation into
  `#styleLintPanel`, debounced 400ms. It is unaffected by this feature other
  than firing again when import changes the editor content — it validates
  *spec conformance*, not conversion quality.
- The `Style` model (`src/models/styles.rs`) has no layer/source-layer
  association, so `source_layer` must always come from the user at import
  time. `Catalog` (`src/models/catalog.rs`) holds `Vec<Layer>` behind
  `get_catalog().await.read().await`; `Layer.name` is the field used
  elsewhere as MapLibre's `source-layer` (confirmed in
  `templates/maplayer.html`), not `Layer.table_name` (the internal DB
  table).
- Admin routes are session-authenticated: `build_admin_styles_routes()` in
  `src/routes.rs` sits under `build_admin_routes()`
  (`.hoop(auth::session_auth_handler)`) and its own
  `.hoop(auth::require_user_admin)`. The separate `/api/admin/...` tree is
  JWT-only, for external API consumers — not used here since the caller is
  the admin page's own browser session.
- No file upload exists anywhere in the codebase today (`grep` for
  `multipart`/`FormData` in `src/` and `templates/admin/` returns nothing).

## Decisions

- **Conversion runs server-side.** The browser reads the picked `.qml`
  locally with `FileReader.readAsText()` and sends the text in a JSON
  `fetch()` body — it is never submitted through the main `<form>`, never
  written to disk, and never held as an actual uploaded file. This also
  fully resolves the "don't keep the file around" requirement: there is no
  file server-side to clean up, because none is ever created.
- **`source_layer` is a dropdown sourced from the layers catalog**
  (`layer.name` from `get_catalog()`), not free text — it's a closed,
  simple value set, consistent with [[feedback-curated-vs-raw-json-editing]]
  (that memory warns against curated pickers over open-ended/expression-rich
  JSON, not against dropdowns for genuinely closed values).
- **Output mode is user-selectable**, mapped 1:1 to `OutputMode`: "MapLibre
  puro" (`Compact`) / "Compatible con QGIS" (`QgisCompatible`).
- **Merge mode is user-selectable, default concatenate.** A radio toggle
  next to the import button: "Concatenar" (default) / "Reemplazar". Both
  modes act **only on the `layers` key** of whatever's currently in the
  editor — `sources`/`version`/other root keys are never touched:
  - Concatenar → `layers_actuales.concat(layers_nuevas)`
  - Reemplazar → drop `layers_actuales`, keep `layers_nuevas`
  - If the editor is empty (`{}`, fresh "new style" page), both modes are
    equivalent: `layers` is set directly.
  After applying, the code dispatches `style-editor-changed` (same as the
  existing buttons) so `styleLintPanel` re-validates automatically.
- **Conversion warnings get their own panel**, separate from
  `styleLintPanel`. They are a different kind of signal (QML→MapLibre
  conversion caveats, e.g. "unsupported renderer") from spec validation
  (MapLibre Style Spec conformance of whatever's currently in the editor).
  Mixing them would conflate "this didn't convert well from QGIS" with
  "this isn't valid MapLibre" under one label.
- **The whole path is strictly optional**, a third prefill mechanism
  alongside `btnFullStyle`/`btnPartialStyle`. Manual raw-JSON editing keeps
  working exactly as today.
- `qml2maplibre` is added as a **path dependency** in `Cargo.toml`:
  `qml2maplibre = { path = "../any2maplibre/qml2maplibre" }` (mvt-rs and
  any2maplibre are sibling directories under `/home/jose/trabajos/mvt-proj/`)
  until it's published to crates.io, at which point the dependency switches
  to a version requirement.

## Components

### 1. `Cargo.toml`

Add `qml2maplibre` as a path dependency.

### 2. Backend endpoint

`POST /admin/styles/convert-qml`, added inside `build_admin_styles_routes()`
in `src/routes.rs` (inherits session auth + `require_user_admin` from the
enclosing routers — no new `.hoop()` needed). Handler lives in
`src/html/admin/styles.rs` alongside `create_style`/`update_style`, and
returns JSON despite the module being otherwise HTML-page-oriented (it's an
AJAX call from those same pages).

- **Request body (JSON):**
  ```json
  { "qml": "<.qml file contents as text>", "source_layer": "layer_name", "mode": "compact" | "qgis" }
  ```
- **Response success (200):**
  ```json
  { "layers": [ /* MapLibreLayer JSON, serde_json::to_value of ConversionResult.layers */ ], "warnings": ["..."] }
  ```
- **Response error (400):** `ConvertError` is mapped to
  `AppError::InvalidInput(message)`, which already renders as
  `{ "error": "..." }` JSON (or an HTML error page, per `Accept` header,
  though this endpoint is only ever called via `fetch()`/JSON).
- No file is written to disk at any point; the handler works entirely with
  the `qml` string in memory.

### 3. UI (`templates/admin/styles/new.html`, `edit.html`)

New block next to the existing `btnFullStyle`/`btnPartialStyle` buttons,
e.g. "Importar desde QML":

- `<input type="file" accept=".qml">` — read via `FileReader`, never bound
  to the main form's `enctype`/submission.
- `<select>` of `source_layer`, populated server-side from
  `get_catalog().await.read().await.layers` (handlers `new_style_page` /
  `edit_style_page` in `src/html/admin/styles.rs` gain a `layers: Vec<...>`
  template field, following the same pattern `NewLayerTemplate` already uses
  for `categories`/`groups`/`databases`).
- `<select>` of output mode: "MapLibre puro" / "Compatible con QGIS".
- Radio: "Concatenar" (checked by default) / "Reemplazar".
- Button "Importar QML" → `fetch('/admin/styles/convert-qml', { method: 'POST', body: JSON.stringify({...}) })`.
- On success: apply the merge logic described above, call
  `editor.set(mergedJson)`, dispatch `style-editor-changed`, and render
  `warnings` (if any) into the new `#qmlImportWarnings` panel; hide that
  panel again on the next manual edit to the editor.
- On error: show the message from the JSON error response in a dedicated
  error slot near the import block (not reusing `#jsonError`, which is
  specifically for the main form's JSON-syntax-on-submit check).

### 4. i18n

New Fluent keys (ES + EN, following [[ftl-locales-convention]]) for: import
button label, source-layer/mode/merge-mode labels, "Concatenar"/"Reemplazar"
option labels, warnings panel title, generic conversion-error prefix.

## Error handling

- `ConvertError::Xml` / `UnexpectedEof` → malformed QML file → 400 with a
  message surfaced verbatim (both are already Spanish-language messages from
  `qml2maplibre`/`thiserror`).
- `ConvertError::NoRules` / `AllRulesUnsupported` → QML parsed but produced
  no usable MapLibre rules → 400, same handling.
- Network/fetch failure (server unreachable, unexpected non-JSON response)
  → generic "no se pudo convertir el archivo" message in the same error
  slot.
- `FileReader` failure (unreadable file) → same error slot, no request
  sent.

## Testing

- **Rust unit tests**: the new endpoint handler — valid QML + valid
  `source_layer` returns 200 with expected layer/warning shape; malformed
  QML returns 400 with `InvalidInput`; missing/unknown `source_layer` is
  accepted as-is (it's just a string passed through to `convert()`, no
  catalog-membership check needed since the dropdown already constrains the
  UI path — server doesn't need to re-validate a value it doesn't otherwise
  use for anything privileged).
- **Manual browser verification** (no JS test infra in repo, per
  [[maplibre-style-validation]] precedent):
  - Import a real `.qml` on `new.html` with an empty editor → layers appear,
    `styleLintPanel` runs and shows valid/errors as appropriate.
  - Import twice with "Concatenar" → layer count doubles, ids may collide
    (acceptable — the toggle exists precisely so the user can choose
    "Reemplazar" instead for that case).
  - Import with "Reemplazar" → previous imported layers gone, other root
    keys (if any) untouched.
  - QML that produces `warnings` → panel shows them, separate from any
    `styleLintPanel` output.
  - Malformed `.qml` → error slot shows a message, editor content
    unchanged.
  - Same flow on `edit.html` with a pre-existing style already loaded into
    the editor.

## Out of scope

- WASM/browser-side conversion (rejected earlier in favor of server-side —
  user's own words: "Prefiero servidor, es más simple").
- Persisting or caching uploaded `.qml` files server-side.
- Validating that the chosen `source_layer` actually matches the layer the
  QML was originally styled for in QGIS — that's the user's responsibility,
  same as writing raw JSON by hand today.
- SLD import (`sld2maplibre` exists in the same workspace but is not part of
  this spec).
