# QML Style Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** let admins convert a QGIS `.qml` style file into MapLibre layer JSON from the `new.html`/`edit.html` style editor, via a new session-authenticated endpoint that calls the `qml2maplibre` crate.

**Architecture:** the browser reads the picked `.qml` locally (`FileReader`, no multipart upload, nothing ever written to disk), POSTs the raw XML text as JSON to `POST /admin/styles/convert-qml`, and the handler calls `qml2maplibre::convert()` in memory and returns the resulting MapLibre layers + conversion warnings as JSON. A shared JS module merges the returned layers into whatever is currently in the JSONEditor (concat or replace, user's choice) and renders warnings in their own panel, separate from the existing `styleLintPanel` spec-validator.

**Tech Stack:** Rust / Salvo / Askama (backend + templates), vanilla JS ES module (frontend), Fluent (i18n).

**Spec:** `docs/superpowers/specs/2026-08-02-qml-style-import-design.md`

## Global Constraints

- `qml2maplibre` is added as a path dependency: `{ path = "../any2maplibre/qml2maplibre" }` (mvt-rs and any2maplibre are sibling directories).
- The `.qml` content is never written to disk server-side — the handler only ever sees it as an in-memory `String` from the JSON request body.
- The new endpoint lives under `build_admin_styles_routes()` (session-authenticated + `require_user_admin`), **not** under `/api/admin/...` (that tree is JWT-only, for external API consumers).
- `source_layer` is a dropdown sourced from `get_catalog().await.read().await.layers` (`Layer.name`), never free text.
- Merge mode defaults to **concatenate**; both merge modes act only on the editor JSON's `layers` key, never `sources`/`version`/other root keys.
- Conversion `warnings` render in their own panel (`#qmlImportWarnings`), never mixed into `#styleLintPanel`.
- Every new Fluent key must be added to **all six** locale files: `en-US.ftl`, `es-AR.ftl`, `es-ES.ftl`, `fr-FR.ftl`, `it-IT.ftl`, `pt-BR.ftl`.
- The feature is strictly additive: `btnFullStyle`/`btnPartialStyle` and manual raw-JSON editing keep working unchanged.

---

### Task 1: Add `qml2maplibre` path dependency

**Files:**
- Modify: `Cargo.toml:40`

**Interfaces:**
- Produces: the `qml2maplibre` crate becomes available as `qml2maplibre::{convert, OutputMode, ConvertError, MapLibreLayer}` to all later tasks.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, after the last line of the `[dependencies]` block (`accept-language = "3.1"`), add:

```toml
qml2maplibre = { path = "../any2maplibre/qml2maplibre" }
```

- [ ] **Step 2: Verify it builds**

Run: `cargo check`
Expected: compiles successfully (this only pulls in the dependency graph; nothing uses the crate yet, so no new warnings about unused imports should appear since it's not `use`d anywhere yet).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add qml2maplibre path dependency"
```

---

### Task 2: Add QML-import Fluent i18n keys to all locales

**Files:**
- Modify: `locales/en-US.ftl:122`
- Modify: `locales/es-AR.ftl:124`
- Modify: `locales/es-ES.ftl:122`
- Modify: `locales/fr-FR.ftl:122`
- Modify: `locales/it-IT.ftl:122`
- Modify: `locales/pt-BR.ftl:122`

**Interfaces:**
- Produces: Fluent keys `qml-import-title`, `qml-import-file-label`, `qml-import-source-layer-label`, `qml-import-mode-label`, `qml-import-mode-compact`, `qml-import-mode-qgis`, `qml-import-merge-label`, `qml-import-merge-concat`, `qml-import-merge-replace`, `qml-import-button`, `qml-import-warnings-title`, `qml-import-error-generic` — consumed by Task 6's templates via `base.translate["<key>"]`.

- [ ] **Step 1: Append the new keys to `locales/en-US.ftl`**

Find the line `style-lint-errors = MapLibre spec errors` and add immediately after it:

```fluent

qml-import-title = Import from QML
qml-import-file-label = QML file
qml-import-source-layer-label = Layer
qml-import-mode-label = Conversion mode
qml-import-mode-compact = MapLibre only
qml-import-mode-qgis = QGIS-compatible
qml-import-merge-label = Merge mode
qml-import-merge-concat = Concatenate
qml-import-merge-replace = Replace
qml-import-button = Import QML
qml-import-warnings-title = Conversion warnings
qml-import-error-generic = Could not convert the QML file
```

- [ ] **Step 2: Append the new keys to `locales/es-AR.ftl`**

Find the line `style-lint-errors = Errores del spec de MapLibre` and add immediately after it:

```fluent

qml-import-title = Importar desde QML
qml-import-file-label = Archivo QML
qml-import-source-layer-label = Capa
qml-import-mode-label = Modo de conversión
qml-import-mode-compact = Solo MapLibre
qml-import-mode-qgis = Compatible con QGIS
qml-import-merge-label = Modo de combinación
qml-import-merge-concat = Concatenar
qml-import-merge-replace = Reemplazar
qml-import-button = Importar QML
qml-import-warnings-title = Avisos de conversión
qml-import-error-generic = No se pudo convertir el archivo QML
```

- [ ] **Step 3: Append the new keys to `locales/es-ES.ftl`**

Find the line `style-lint-errors = Errores de la especificación de MapLibre` and add immediately after it:

```fluent

qml-import-title = Importar desde QML
qml-import-file-label = Archivo QML
qml-import-source-layer-label = Capa
qml-import-mode-label = Modo de conversión
qml-import-mode-compact = Solo MapLibre
qml-import-mode-qgis = Compatible con QGIS
qml-import-merge-label = Modo de combinación
qml-import-merge-concat = Concatenar
qml-import-merge-replace = Reemplazar
qml-import-button = Importar QML
qml-import-warnings-title = Avisos de conversión
qml-import-error-generic = No se pudo convertir el archivo QML
```

- [ ] **Step 4: Append the new keys to `locales/fr-FR.ftl`**

Find the line `style-lint-errors = Erreurs de la spécification MapLibre` and add immediately after it:

```fluent

qml-import-title = Importer depuis QML
qml-import-file-label = Fichier QML
qml-import-source-layer-label = Couche
qml-import-mode-label = Mode de conversion
qml-import-mode-compact = MapLibre uniquement
qml-import-mode-qgis = Compatible QGIS
qml-import-merge-label = Mode de fusion
qml-import-merge-concat = Concaténer
qml-import-merge-replace = Remplacer
qml-import-button = Importer QML
qml-import-warnings-title = Avertissements de conversion
qml-import-error-generic = Impossible de convertir le fichier QML
```

- [ ] **Step 5: Append the new keys to `locales/it-IT.ftl`**

Find the line `style-lint-errors = Errori della specifica MapLibre` and add immediately after it:

```fluent

qml-import-title = Importa da QML
qml-import-file-label = File QML
qml-import-source-layer-label = Layer
qml-import-mode-label = Modalità di conversione
qml-import-mode-compact = Solo MapLibre
qml-import-mode-qgis = Compatibile con QGIS
qml-import-merge-label = Modalità di unione
qml-import-merge-concat = Concatena
qml-import-merge-replace = Sostituisci
qml-import-button = Importa QML
qml-import-warnings-title = Avvisi di conversione
qml-import-error-generic = Impossibile convertire il file QML
```

- [ ] **Step 6: Append the new keys to `locales/pt-BR.ftl`**

Find the line `style-lint-errors = Erros da especificação MapLibre` and add immediately after it:

```fluent

qml-import-title = Importar de QML
qml-import-file-label = Arquivo QML
qml-import-source-layer-label = Camada
qml-import-mode-label = Modo de conversão
qml-import-mode-compact = Somente MapLibre
qml-import-mode-qgis = Compatível com QGIS
qml-import-merge-label = Modo de mesclagem
qml-import-merge-concat = Concatenar
qml-import-merge-replace = Substituir
qml-import-button = Importar QML
qml-import-warnings-title = Avisos de conversão
qml-import-error-generic = Não foi possível converter o arquivo QML
```

- [ ] **Step 7: Verify the app still builds and loads locales**

Run: `cargo check`
Expected: compiles (locale files are embedded via `include_dir!` at compile time, so a malformed `.ftl` would still compile but log an `I18n Error`/`I18n Warning` at startup — run `cargo run` briefly and confirm no `I18n Error`/`I18n Warning` lines appear in the log, then stop it).

- [ ] **Step 8: Commit**

```bash
git add locales/en-US.ftl locales/es-AR.ftl locales/es-ES.ftl locales/fr-FR.ftl locales/it-IT.ftl locales/pt-BR.ftl
git commit -m "i18n: add QML style import strings for all locales"
```

---

### Task 3: `parse_output_mode` pure function

**Files:**
- Modify: `src/html/admin/styles.rs`

**Interfaces:**
- Produces: `fn parse_output_mode(mode: &str) -> AppResult<qml2maplibre::OutputMode>` (private to the module) — consumed by Task 4's `convert_qml` handler.

- [ ] **Step 1: Write the failing tests**

At the bottom of `src/html/admin/styles.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_mode_accepts_compact() {
        assert_eq!(
            parse_output_mode("compact").unwrap(),
            qml2maplibre::OutputMode::Compact
        );
    }

    #[test]
    fn parse_output_mode_accepts_qgis() {
        assert_eq!(
            parse_output_mode("qgis").unwrap(),
            qml2maplibre::OutputMode::QgisCompatible
        );
    }

    #[test]
    fn parse_output_mode_rejects_unknown_value() {
        assert!(parse_output_mode("bogus").is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib html::admin::styles::tests`
Expected: FAIL to compile — `parse_output_mode` is not defined.

- [ ] **Step 3: Implement `parse_output_mode`**

Above the `#[cfg(test)]` block (e.g. right after the existing `delete_style` handler), add:

```rust
fn parse_output_mode(mode: &str) -> AppResult<qml2maplibre::OutputMode> {
    match mode {
        "compact" => Ok(qml2maplibre::OutputMode::Compact),
        "qgis" => Ok(qml2maplibre::OutputMode::QgisCompatible),
        other => Err(AppError::InvalidInput(format!(
            "unknown conversion mode '{other}'"
        ))),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib html::admin::styles::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/html/admin/styles.rs
git commit -m "feat: add QML conversion output-mode parsing"
```

---

### Task 4: `convert_qml` endpoint

**Files:**
- Modify: `src/html/admin/styles.rs`
- Modify: `src/routes.rs:149-158`

**Interfaces:**
- Consumes: `parse_output_mode(mode: &str) -> AppResult<qml2maplibre::OutputMode>` (Task 3); `qml2maplibre::convert(qml_xml: &str, source_layer: &str, mode: OutputMode) -> Result<ConversionResult, ConvertError>` where `ConversionResult { layers: Vec<MapLibreLayer>, warnings: Vec<String> }`.
- Produces: `POST /admin/styles/convert-qml` — request body `{ "qml": string, "source_layer": string, "mode": "compact" | "qgis" }`, response `200 { "layers": [...MapLibreLayer JSON...], "warnings": [string] }` or `400 { "error": string }`. Consumed by Task 6's `qml-import.js`.

- [ ] **Step 1: Write the failing tests**

In `src/html/admin/styles.rs`, inside the `mod tests` block added in Task 3, add (keep the existing `parse_output_mode_*` tests):

```rust
    const SINGLESYMBOL_LINE_QML: &str = r#"<!DOCTYPE qgis PUBLIC 'http://mrcc.com/qgis.dtd' 'SYSTEM'>
<qgis>
  <renderer-v2 enableorderby="0" type="singleSymbol" symbollevels="0" forceraster="0">
    <symbols>
      <symbol type="line" alpha="1" force_rhr="0" name="0" clip_to_extent="1">
        <layer locked="0" pass="0" enabled="1" class="SimpleLine">
          <prop v="square" k="capstyle"/>
          <prop v="bevel" k="joinstyle"/>
          <prop v="9,113,185,255" k="line_color"/>
          <prop v="solid" k="line_style"/>
          <prop v="2" k="line_width"/>
          <prop v="Pixel" k="line_width_unit"/>
          <data_defined_properties>
            <Option type="Map">
              <Option type="QString" value="" name="name"/>
            </Option>
          </data_defined_properties>
        </layer>
      </symbol>
    </symbols>
  </renderer-v2>
</qgis>
"#;

    #[tokio::test]
    async fn convert_qml_returns_layers_for_valid_qml() {
        use salvo::test::{ResponseExt, TestClient};

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": SINGLESYMBOL_LINE_QML,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let mut resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
        let json: serde_json::Value = resp.take_json().await.unwrap();
        let layers = json["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0]["type"], "line");
        assert_eq!(layers[0]["source-layer"], "test_layer");
    }

    #[tokio::test]
    async fn convert_qml_rejects_malformed_xml() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": "<not valid xml",
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_qml_rejects_unknown_mode() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": SINGLESYMBOL_LINE_QML,
            "source_layer": "test_layer",
            "mode": "bogus",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib html::admin::styles::tests`
Expected: FAIL to compile — `convert_qml` is not defined.

- [ ] **Step 3: Implement the request struct and handler**

In `src/html/admin/styles.rs`, after the `NewStyle<'a>` struct definition, add:

```rust
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct ConvertQmlRequest {
    qml: String,
    source_layer: String,
    mode: String,
}
```

Then, after the `delete_style` handler (and after `parse_output_mode` from Task 3), add:

```rust
#[handler]
pub async fn convert_qml(res: &mut Response, data: ConvertQmlRequest) -> AppResult<()> {
    let mode = parse_output_mode(&data.mode)?;
    let result = qml2maplibre::convert(&data.qml, &data.source_layer, mode)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    res.render(Json(serde_json::json!({
        "layers": result.layers,
        "warnings": result.warnings,
    })));
    Ok(())
}
```

- [ ] **Step 4: Wire the route**

In `src/routes.rs`, in `build_admin_styles_routes()` (around line 149), add the new push:

```rust
fn build_admin_styles_routes() -> Router {
    Router::with_path("styles")
        .hoop(auth::require_user_admin)
        .get(html::admin::styles::list_styles)
        .push(Router::with_path("new").get(html::admin::styles::new_style_page))
        .push(Router::with_path("create").post(html::admin::styles::create_style))
        .push(Router::with_path("edit/{id}").get(html::admin::styles::edit_style_page))
        .push(Router::with_path("update").post(html::admin::styles::update_style))
        .push(Router::with_path("delete/{id}").get(html::admin::styles::delete_style))
        .push(Router::with_path("convert-qml").post(html::admin::styles::convert_qml))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib html::admin::styles::tests`
Expected: PASS (6 tests total: 3 from Task 3, 3 new).

- [ ] **Step 6: Commit**

```bash
git add src/html/admin/styles.rs src/routes.rs
git commit -m "feat: add POST /admin/styles/convert-qml endpoint"
```

---

### Task 5: Expose the layers catalog to the style page templates

**Files:**
- Modify: `src/html/admin/styles.rs`

**Interfaces:**
- Consumes: `get_catalog() -> &'static RwLock<Catalog>` where `Catalog { layers: Vec<Layer> }` and `Layer { name: String, ... }` (`src/models/catalog.rs`); `Layer::sort_by_category_and_name(layers: &mut [Layer])`.
- Produces: `NewStyleTemplate.layers: Vec<Layer>` and `EditStyleTemplate.layers: Vec<Layer>` — consumed by Task 6's template markup as `{% for layer in layers %}`.

- [ ] **Step 1: Import `get_catalog` and `Layer`**

In `src/html/admin/styles.rs`, change the `use crate::{...}` block at the top:

```rust
use crate::{
    auth::User,
    error::{AppError, AppResult},
    get_categories, get_catalog,
    html::utils::{BaseTemplateData, make_base},
    models::{category::Category, catalog::Layer, styles::Style},
};
```

- [ ] **Step 2: Add the `layers` field to both templates**

```rust
#[derive(Template)]
#[template(path = "admin/styles/new.html")]
struct NewStyleTemplate {
    categories: Vec<Category>,
    layers: Vec<Layer>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/styles/edit.html")]
struct EditStyleTemplate {
    style: Style,
    style_json_safe: String,
    categories: Vec<Category>,
    layers: Vec<Layer>,
    base: BaseTemplateData,
}
```

- [ ] **Step 3: Populate `layers` in both handlers**

```rust
#[handler]
pub async fn new_style_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let categories = get_categories().await.read().await;
    let mut layers = get_catalog().await.read().await.layers.clone();
    Layer::sort_by_category_and_name(&mut layers);
    let template = NewStyleTemplate {
        categories: (categories).to_vec(),
        layers,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_style_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    let categories = get_categories().await.read().await;
    let mut layers = get_catalog().await.read().await.layers.clone();
    Layer::sort_by_category_and_name(&mut layers);
    // Escape "</" so the JSON can't break out of the <script type="application/json">
    // block it's embedded in (e.g. a style value containing "</script>"); JSON.parse
    // unescapes "\/" back to "/" so the parsed value is unaffected.
    let style_json_safe = style.style.replace("</", "<\\/");
    let template = EditStyleTemplate {
        style: style.clone(),
        style_json_safe,
        categories: (categories).to_vec(),
        layers,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: PASS. `new.html`/`edit.html` don't reference `layers` in their templates yet (that's Task 6) — an unused struct field doesn't fail an Askama build, only a template referencing an *undefined* variable would.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test`
Expected: PASS, no regressions (existing style tests in `src/models/styles.rs` and the new ones from Tasks 3-4 all still pass).

- [ ] **Step 6: Commit**

```bash
git add src/html/admin/styles.rs
git commit -m "feat: pass layers catalog to style editor templates"
```

---

### Task 6: QML import UI — markup, shared JS module, script includes

**Files:**
- Modify: `templates/admin/styles/new.html:7,80-81`
- Modify: `templates/admin/styles/edit.html:7,79-81`
- Create: `static/js/qml-import.js`

**Interfaces:**
- Consumes: `POST /admin/styles/convert-qml` (Task 4); `base.translate["qml-import-*"]` (Task 2); `layers` template variable (Task 5); the `style-editor-ready` / `style-editor-changed` event contract already used by `static/js/style-validator.js` (`event.detail.editor` is the `JSONEditor` instance).
- Produces: DOM elements `#qmlFileInput`, `#qmlSourceLayer`, `#qmlOutputMode`, `input[name="qmlMergeMode"]`, `#btnImportQml`, `#qmlImportError`, `#qmlImportWarnings`, `#qmlImportPanel` — all consumed only by `qml-import.js` itself (no other task depends on these IDs).

- [ ] **Step 1: Add the script include to `templates/admin/styles/new.html`**

In the `{% block head %}` section, after the existing `style-validator.js` script tag (line 7):

```html
  <script type="module" src="/static/js/style-validator.js"></script>
  <script type="module" src="/static/js/qml-import.js"></script>
```

- [ ] **Step 2: Add the import panel markup to `templates/admin/styles/new.html`**

Between the closing `</div>` of the "JSON Editor for Style" block (line 80) and the `<div id="jsonError" ...>` block (line 82), insert:

```html
      <!-- QML import -->
      <div class="mb-4 border-t pt-4 mt-4">
        <label class="label">{{ base.translate["qml-import-title"] }}</label>
        <div class="mt-1 flex flex-wrap gap-3 items-end" id="qmlImportPanel"
             data-msg-generic-error="{{ base.translate["qml-import-error-generic"] }}"
             data-msg-warnings-title="{{ base.translate["qml-import-warnings-title"] }}">
          <div>
            <label class="label text-xs">{{ base.translate["qml-import-file-label"] }}</label>
            <input type="file" id="qmlFileInput" accept=".qml" class="input">
          </div>
          <div>
            <label class="label text-xs">{{ base.translate["qml-import-source-layer-label"] }}</label>
            <select id="qmlSourceLayer" class="input">
              {% for layer in layers %}
                <option value="{{ layer.name }}">{{ layer.name }}</option>
              {% endfor %}
            </select>
          </div>
          <div>
            <label class="label text-xs">{{ base.translate["qml-import-mode-label"] }}</label>
            <select id="qmlOutputMode" class="input">
              <option value="compact">{{ base.translate["qml-import-mode-compact"] }}</option>
              <option value="qgis">{{ base.translate["qml-import-mode-qgis"] }}</option>
            </select>
          </div>
          <div>
            <span class="label text-xs">{{ base.translate["qml-import-merge-label"] }}</span>
            <label class="flex items-center gap-1">
              <input type="radio" name="qmlMergeMode" value="concat" checked>
              {{ base.translate["qml-import-merge-concat"] }}
            </label>
            <label class="flex items-center gap-1">
              <input type="radio" name="qmlMergeMode" value="replace">
              {{ base.translate["qml-import-merge-replace"] }}
            </label>
          </div>
          <button type="button" id="btnImportQml" class="button">
            <span class="icon__small mr-2">
              <i class="fas fa-file-import"></i>
            </span>
            {{ base.translate["qml-import-button"] }}
          </button>
        </div>
        <div id="qmlImportError" class="text-red-500 text-sm mt-2" style="display: none;"></div>
        <div id="qmlImportWarnings" class="mt-2" style="display: none;"></div>
      </div>
```

- [ ] **Step 3: Apply the same two changes to `templates/admin/styles/edit.html`**

Same script tag after line 7's `style-validator.js` include, and the same import-panel markup block inserted between the closing `</div>` of the "JSON Editor for Style" block (line 79) and the `<div id="jsonError" ...>` block (line 81). The markup is identical — `edit.html` already has `layers` in scope from Task 5.

- [ ] **Step 4: Create `static/js/qml-import.js`**

```javascript
// QML → MapLibre import panel, shared by new.html and edit.html.
//
// Gets a handle on the page's JSONEditor via the same `style-editor-ready`
// CustomEvent that style-validator.js listens for (detail.editor). On
// import, POSTs the file's text content (read client-side, never uploaded
// as a real file) to /admin/styles/convert-qml, then merges the returned
// layers into the editor per the selected merge mode.

let editorRef = null;

document.addEventListener('style-editor-ready', (event) => {
  editorRef = event.detail.editor;
});

function readFileAsText(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}

// Exported for potential reuse/testing: merges newLayers into existingJson's
// `layers` key only. Never touches `sources`/`version`/other root keys.
export function mergeLayers(existingJson, newLayers, mode) {
  const base = existingJson && typeof existingJson === 'object' && !Array.isArray(existingJson)
    ? { ...existingJson }
    : {};
  const existingLayers = Array.isArray(base.layers) ? base.layers : [];
  base.layers = mode === 'replace' ? newLayers : existingLayers.concat(newLayers);
  return base;
}

function renderWarnings(panel, warnings) {
  panel.innerHTML = '';
  if (!warnings || warnings.length === 0) {
    panel.style.display = 'none';
    return;
  }
  const title = document.createElement('p');
  title.className = 'text-yellow-600 text-sm font-bold';
  title.textContent = panel.dataset.msgWarningsTitle || 'Conversion warnings';
  panel.appendChild(title);
  const list = document.createElement('ul');
  list.className = 'text-yellow-600 text-sm list-disc pl-5';
  for (const warning of warnings) {
    const item = document.createElement('li');
    item.textContent = warning;
    list.appendChild(item);
  }
  panel.appendChild(list);
  panel.style.display = 'block';
}

document.addEventListener('DOMContentLoaded', () => {
  const button = document.getElementById('btnImportQml');
  const importPanel = document.getElementById('qmlImportPanel');
  if (!button || !importPanel) {
    return;
  }

  const fileInput = document.getElementById('qmlFileInput');
  const sourceLayerSelect = document.getElementById('qmlSourceLayer');
  const modeSelect = document.getElementById('qmlOutputMode');
  const errorSlot = document.getElementById('qmlImportError');
  const warningsPanel = document.getElementById('qmlImportWarnings');
  const genericErrorMsg = importPanel.dataset.msgGenericError || 'Could not convert the QML file';

  function showError(message) {
    errorSlot.textContent = message;
    errorSlot.style.display = 'block';
  }

  function clearError() {
    errorSlot.style.display = 'none';
  }

  button.addEventListener('click', async () => {
    clearError();
    warningsPanel.style.display = 'none';

    const file = fileInput.files[0];
    if (!file || !editorRef) {
      return;
    }

    let qmlText;
    try {
      qmlText = await readFileAsText(file);
    } catch (e) {
      showError(genericErrorMsg);
      return;
    }

    const mergeMode = document.querySelector('input[name="qmlMergeMode"]:checked').value;

    let response;
    try {
      response = await fetch('/admin/styles/convert-qml', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          qml: qmlText,
          source_layer: sourceLayerSelect.value,
          mode: modeSelect.value,
        }),
      });
    } catch (e) {
      showError(genericErrorMsg);
      return;
    }

    if (!response.ok) {
      let message = genericErrorMsg;
      try {
        const errJson = await response.json();
        if (errJson.error) {
          message = errJson.error;
        }
      } catch (e) {
        // Non-JSON error body: keep the generic message.
      }
      showError(message);
      return;
    }

    const result = await response.json();
    let currentJson;
    try {
      currentJson = editorRef.get();
    } catch (e) {
      currentJson = {};
    }

    editorRef.set(mergeLayers(currentJson, result.layers, mergeMode));
    document.dispatchEvent(new Event('style-editor-changed'));
    renderWarnings(warningsPanel, result.warnings);
  });
});
```

- [ ] **Step 5: Verify the app builds and the pages render**

Run: `cargo check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add templates/admin/styles/new.html templates/admin/styles/edit.html static/js/qml-import.js
git commit -m "feat: add QML import panel to the style editor UI"
```

---

### Task 7: Manual end-to-end verification

No automated test infrastructure exists for the admin UI's JS (matches the precedent set by `style-validator.js` — see `docs/superpowers/specs/2026-07-12-maplibre-style-validation-design.md`). Verify by hand:

- [ ] **Step 1: Start the server**

Run: `cargo run`
Expected: starts without `I18n Error`/`I18n Warning` log lines, no panics.

- [ ] **Step 2: New style, empty editor, concatenate mode**

Log in as an admin user, go to `/admin/styles/new`. Pick `/home/jose/trabajos/mvt-proj/any2maplibre/qml2maplibre/tests/fixtures/singlesymbol_line.qml` as the QML file, select any layer from the "Layer" dropdown, mode "MapLibre only", merge mode "Concatenate" (default), click "Import QML".
Expected: the JSONEditor now shows `{"layers": [{...one "line" layer...}]}`, `styleLintPanel` re-runs and shows a result, no error/warnings panel visible (this fixture produces no warnings).

- [ ] **Step 3: Import a second time with "Concatenate"**

Click "Import QML" again with the same inputs.
Expected: `layers` array now has 2 entries (duplicated ids are expected/acceptable per the design).

- [ ] **Step 4: Switch to "Replace" and import again**

Select merge mode "Replace", click "Import QML" again.
Expected: `layers` array goes back down to 1 entry (the two duplicates are gone, replaced by the new import).

- [ ] **Step 5: Malformed QML**

Create a throwaway text file with content `not valid xml at all` (extension `.qml`), pick it, click "Import QML".
Expected: `#qmlImportError` shows a message, editor content is unchanged from before the click.

- [ ] **Step 6: Existing style, edit.html**

Open `/admin/styles/edit/<id>` for any existing style with content already in the editor. Import a `.qml` with merge mode "Concatenate".
Expected: previously-existing `layers` (and any other root keys like `version`/`sources` if the style is a full map style) are preserved; new layers are appended.

- [ ] **Step 7: Confirm the raw-JSON path is untouched**

On `/admin/styles/new`, click `btnFullStyle` / `btnPartialStyle` as before (unrelated to this feature).
Expected: unchanged behavior — these still work exactly as before this feature was added.

- [ ] **Step 8: Stop the server**

Stop the `cargo run` process.
