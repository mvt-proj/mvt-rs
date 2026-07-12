# MapLibre Style Schema Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate styles against the MapLibre Style Spec live in the admin style editor (warn, never block), and reject unparseable JSON server-side.

**Architecture:** A self-wiring ES module (`static/js/style-validator.js`) imports the official `@maplibre/maplibre-gl-style-spec` validator from a pinned CDN URL and communicates with the existing JSONEditor pages via two DOM events (`style-editor-ready`, `style-editor-changed`), rendering results into a `#styleLintPanel` div. Layer fragments (styles without `version`) are wrapped in a synthetic style before validation. Server-side, a pure function `validate_style_json` rejects unparseable JSON in the create/update handlers with a 400.

**Tech Stack:** Rust (Salvo, serde_json), vanilla ES modules, JSONEditor 10.x, `@maplibre/maplibre-gl-style-spec@25.0.2` via jsdelivr, Fluent i18n.

**Spec:** `docs/superpowers/specs/2026-07-12-maplibre-style-validation-design.md`

## Global Constraints

- Validator CDN URL, pinned: `https://cdn.jsdelivr.net/npm/@maplibre/maplibre-gl-style-spec@25.0.2/dist/index.mjs`
- Spec errors warn but never block saving/applying; only syntactically invalid JSON blocks submission (existing behavior, unchanged).
- If the CDN import fails, the editor must keep working exactly as today (no lint panel, no JS errors breaking the page).
- Static files are embedded at compile time (`include_dir!` in `src/routes.rs:21`) and served under `/static/` — adding a JS file requires `cargo build` to take effect.
- All user-visible panel strings go through Fluent (`locales/en-US.ftl`, `locales/es-AR.ftl`, `locales/it-IT.ftl`).
- Repo rule: commit locally, never push.

---

### Task 1: Server-side JSON check (`validate_style_json`)

**Files:**
- Modify: `src/services/utils.rs` (add function after `normalize_name`, ~line 335)
- Modify: `src/services/tests.rs` (add tests to existing `mod tests`)
- Modify: `src/html/admin/styles.rs` (call it in `create_style` at ~line 92 and `update_style` at ~line 122)

**Interfaces:**
- Consumes: `AppError::InvalidInput(String)` (exists in `src/error.rs:87`, already maps to HTTP 400 via `status_code()`), `AppResult<T>`.
- Produces: `pub fn validate_style_json(style: &str) -> AppResult<()>` in `crate::services::utils` — Task 5 relies on the 400 behavior existing.

- [ ] **Step 1: Write the failing tests**

In `src/services/tests.rs`, inside the existing `mod tests`, extend the imports and add three tests:

```rust
// change the existing import line to include validate_style_json:
use crate::services::utils::{normalize_name, validate_filter, validate_style_json};
// add this import next to it:
use crate::error::AppError;
```

```rust
#[test]
fn test_validate_style_json_accepts_valid_json() {
    assert!(validate_style_json("{}").is_ok());
    assert!(validate_style_json(r#"{"layers":[{"id":"a"}]}"#).is_ok());
    assert!(validate_style_json(r#"{"version":8,"sources":{},"layers":[]}"#).is_ok());
}

#[test]
fn test_validate_style_json_rejects_invalid_json() {
    assert!(validate_style_json("").is_err());
    assert!(validate_style_json("{not json").is_err());
    assert!(validate_style_json(r#"{"layers": [}"#).is_err());
}

#[test]
fn test_validate_style_json_error_is_invalid_input() {
    match validate_style_json("nope}") {
        Err(AppError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test validate_style_json`
Expected: compile error — `validate_style_json` not found in `crate::services::utils`.

- [ ] **Step 3: Write the implementation**

In `src/services/utils.rs`, after `normalize_name`:

```rust
/// Validates that a style payload is parseable JSON before persisting it.
/// Full MapLibre spec validation happens client-side; this is the server's
/// defense in depth so a broken style can never reach the database (where
/// `Style::to_json()` would silently degrade it to `{}`).
pub fn validate_style_json(style: &str) -> AppResult<()> {
    serde_json::from_str::<serde_json::Value>(style)
        .map(|_| ())
        .map_err(|e| AppError::InvalidInput(format!("style is not valid JSON: {e}")))
}
```

(`AppError`, `AppResult` and `serde_json` are already in scope/deps in this file.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test validate_style_json`
Expected: 3 passed.

- [ ] **Step 5: Wire into the handlers**

In `src/html/admin/styles.rs`, add to `create_style`, as the first statement of the function body (before the `Category::from_id` match):

```rust
if let Err(err) = crate::services::utils::validate_style_json(&style_form.style) {
    res.status_code(StatusCode::BAD_REQUEST);
    return Err(err);
}
```

Add the identical block to `update_style`, right after the `let category = match ...` block (so not-found on `id` still wins over bad JSON).

- [ ] **Step 6: Run the full test suite**

Run: `cargo test`
Expected: all tests pass, no new warnings from `cargo build`.

- [ ] **Step 7: Commit**

```bash
git add src/services/utils.rs src/services/tests.rs src/html/admin/styles.rs
git commit -m "feat(styles): reject unparseable style JSON on create/update"
```

---

### Task 2: Validator module and i18n keys

**Files:**
- Create: `static/js/style-validator.js`
- Modify: `locales/en-US.ftl`, `locales/es-AR.ftl`, `locales/it-IT.ftl` (add two keys each, next to the existing `invalid-json` key ~line 120–122)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: the module self-wires via DOM events. Tasks 3–4 rely on this exact contract:
  - Listens for `style-editor-ready` — a `CustomEvent` on `document` with `detail.editor` = the JSONEditor instance. Runs a first validation immediately.
  - Listens for `style-editor-changed` — a plain `Event` on `document`, dispatched on every text change; validation is debounced 400 ms.
  - Renders into `<div id="styleLintPanel">`, reading display strings from its `data-msg-valid` / `data-msg-errors` attributes.
  - Also exports `validateStyle(json) -> [{path, message}]` (exported for potential reuse; the event wiring is the primary interface).
  - Fluent keys produced: `style-lint-valid`, `style-lint-errors`.

- [ ] **Step 1: Create `static/js/style-validator.js`**

```js
// Live MapLibre style-spec validation for the admin style editor.
//
// Self-wiring: the editor page dispatches `style-editor-ready` (CustomEvent
// with detail.editor = JSONEditor instance) once, and `style-editor-changed`
// on every edit. This module listens, validates (debounced) and renders into
// #styleLintPanel. Spec errors only warn — saving is never blocked here.
// If the CDN import fails the module never executes and the editor keeps
// working without lint feedback.

import { validateStyleMin } from 'https://cdn.jsdelivr.net/npm/@maplibre/maplibre-gl-style-spec@25.0.2/dist/index.mjs';

const DUMMY_SOURCE = '__mvt_dummy_source__';
const DUMMY_GLYPHS = 'https://demotiles.maplibre.org/font/{fontstack}/{range}.pbf';
const DUMMY_SPRITE = 'https://demotiles.maplibre.org/styles/osm-bright-gl-style/sprite';
const DEBOUNCE_MS = 400;

// validateStyleMin messages look like "layers[0].paint.fill-color: <reason>";
// split them into path + reason for display.
function toDisplayError(error) {
  const message = error.message || String(error);
  const sep = message.indexOf(': ');
  if (sep === -1) {
    return { path: '', message };
  }
  return { path: message.slice(0, sep), message: message.slice(sep + 2) };
}

// Layer fragments ({"layers": [...]} without "version") are not valid
// standalone styles: wrap them in a synthetic style so the official
// validator can run. Layer indices are preserved, so error paths keep
// pointing at the user's original JSON.
function wrapFragment(fragment) {
  if (!Array.isArray(fragment.layers)) {
    return null;
  }
  const sources = {};
  const layers = fragment.layers.map((layer) => {
    const copy = structuredClone(layer);
    // The fragment convention allows omitting "source" (the consuming
    // client injects it), so a dummy one is added to satisfy the spec.
    if (copy && typeof copy === 'object' && !('source' in copy)) {
      copy.source = DUMMY_SOURCE;
    }
    if (copy && typeof copy.source === 'string') {
      sources[copy.source] = { type: 'vector' };
    }
    return copy;
  });
  if (!(DUMMY_SOURCE in sources)) {
    sources[DUMMY_SOURCE] = { type: 'vector' };
  }
  return {
    version: 8,
    glyphs: DUMMY_GLYPHS,
    sprite: DUMMY_SPRITE,
    sources,
    layers,
  };
}

export function validateStyle(json) {
  try {
    if (json && typeof json === 'object' && 'version' in json) {
      return validateStyleMin(json).map(toDisplayError);
    }
    const wrapped = wrapFragment(json ?? {});
    if (wrapped === null) {
      return [{ path: 'layers', message: 'a partial style must contain a "layers" array' }];
    }
    return validateStyleMin(wrapped)
      .map(toDisplayError)
      .filter((e) => !e.message.includes(DUMMY_SOURCE) && !e.path.includes(DUMMY_SOURCE));
  } catch (err) {
    console.warn('style validation skipped:', err);
    return [];
  }
}

function renderPanel(panel, errors) {
  panel.innerHTML = '';
  panel.style.display = 'block';
  if (errors.length === 0) {
    const ok = document.createElement('p');
    ok.className = 'text-green-600 text-sm';
    ok.textContent = '✓ ' + (panel.dataset.msgValid || 'Style is valid');
    panel.appendChild(ok);
    return;
  }
  const title = document.createElement('p');
  title.className = 'text-red-500 text-sm font-bold';
  title.textContent = `${panel.dataset.msgErrors || 'Style spec errors'} (${errors.length}):`;
  panel.appendChild(title);
  const list = document.createElement('ul');
  list.className = 'text-red-500 text-sm list-disc pl-5';
  for (const error of errors.slice(0, 50)) {
    const item = document.createElement('li');
    item.textContent = error.path ? `${error.path}: ${error.message}` : error.message;
    list.appendChild(item);
  }
  panel.appendChild(list);
}

document.addEventListener('style-editor-ready', (event) => {
  const editor = event.detail.editor;
  const panel = document.getElementById('styleLintPanel');
  if (!editor || !panel) {
    return;
  }

  let timer = null;
  const run = () => {
    let json;
    try {
      json = editor.get();
    } catch {
      // Unparseable JSON: syntax feedback is #jsonError's job, not ours.
      panel.style.display = 'none';
      return;
    }
    // An empty object (fresh "new style" page) has nothing to lint yet.
    if (json && typeof json === 'object' && !Array.isArray(json) && Object.keys(json).length === 0) {
      panel.style.display = 'none';
      return;
    }
    renderPanel(panel, validateStyle(json));
  };

  document.addEventListener('style-editor-changed', () => {
    clearTimeout(timer);
    timer = setTimeout(run, DEBOUNCE_MS);
  });

  run();
});
```

- [ ] **Step 2: Add the Fluent keys**

In `locales/en-US.ftl`, right after the `invalid-json` line (~120):

```ftl
style-lint-valid = The style is valid according to the MapLibre spec
style-lint-errors = MapLibre spec errors
```

In `locales/es-AR.ftl`, right after `invalid-json` (~122):

```ftl
style-lint-valid = El estilo es válido según el spec de MapLibre
style-lint-errors = Errores del spec de MapLibre
```

In `locales/it-IT.ftl`, right after `invalid-json` (~120):

```ftl
style-lint-valid = Lo stile è valido secondo la specifica MapLibre
style-lint-errors = Errori della specifica MapLibre
```

- [ ] **Step 3: Verify the build embeds the new file**

Run: `cargo build`
Expected: compiles cleanly (`include_dir!` picks up `static/js/style-validator.js`). JS behavior itself is verified in the browser in Task 5.

- [ ] **Step 4: Commit**

```bash
git add static/js/style-validator.js locales/en-US.ftl locales/es-AR.ftl locales/it-IT.ftl
git commit -m "feat(styles): add MapLibre style-spec validator module and i18n keys"
```

---

### Task 3: Wire the edit page

**Files:**
- Modify: `templates/admin/styles/edit.html`

**Interfaces:**
- Consumes: the event/DOM contract from Task 2 (`style-editor-ready` with `detail.editor`, `style-editor-changed`, `#styleLintPanel` with `data-msg-valid`/`data-msg-errors`); Fluent keys `style-lint-valid`, `style-lint-errors`.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Load the module**

In the `{% block head %}` of `templates/admin/styles/edit.html`, after the JSONEditor `<link>` tag, add:

```html
<script type="module" src="/static/js/style-validator.js"></script>
```

- [ ] **Step 2: Add the lint panel**

Right after the existing `#jsonError` div:

```html
<div id="styleLintPanel" class="mb-4" style="display: none;"
     data-msg-valid="{{ base.translate["style-lint-valid"] }}"
     data-msg-errors="{{ base.translate["style-lint-errors"] }}"></div>
```

- [ ] **Step 3: Dispatch the events from the inline script**

Change the JSONEditor constructor to notify on edits:

```js
editor = new JSONEditor(container, {
  modes: ['code', 'tree', 'text', 'preview'],
  mode: 'code',
  onChangeText: () => document.dispatchEvent(new Event('style-editor-changed')),
});
```

And immediately after the existing `try { currentStyle = JSON.parse(...); editor.set(currentStyle); } catch ...` block, add:

```js
document.dispatchEvent(new CustomEvent('style-editor-ready', { detail: { editor } }));
```

(Module scripts execute before `DOMContentLoaded` fires, so the listener is guaranteed to be registered before this dispatch runs.)

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: compiles cleanly (Askama recompiles the template; the two new Fluent keys exist from Task 2).

- [ ] **Step 5: Commit**

```bash
git add templates/admin/styles/edit.html
git commit -m "feat(styles): live MapLibre spec validation on the edit-style page"
```

---

### Task 4: Wire the new-style page

**Files:**
- Modify: `templates/admin/styles/new.html`

**Interfaces:**
- Consumes: same Task 2 contract as Task 3.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Load the module**

In the `{% block head %}` of `templates/admin/styles/new.html`, after the JSONEditor `<link>` tag, add:

```html
<script type="module" src="/static/js/style-validator.js"></script>
```

- [ ] **Step 2: Add the lint panel**

Right after the existing `#jsonError` div:

```html
<div id="styleLintPanel" class="mb-4" style="display: none;"
     data-msg-valid="{{ base.translate["style-lint-valid"] }}"
     data-msg-errors="{{ base.translate["style-lint-errors"] }}"></div>
```

- [ ] **Step 3: Dispatch the events from the inline script**

Change the JSONEditor constructor (note: here it is `const editor = ...`):

```js
const editor = new JSONEditor(container, {
  modes: ['code', 'tree', 'text', 'preview'],
  mode: 'code',
  onChangeText: () => document.dispatchEvent(new Event('style-editor-changed')),
});
```

And after the existing final `editor.set({});` line, add:

```js
document.dispatchEvent(new CustomEvent('style-editor-ready', { detail: { editor } }));
```

(The initial `{}` is deliberately not linted — the module hides the panel for an empty object. Clicking "Insert full example" / "Insert partial example" triggers `onChangeText`, which lints the inserted example.)

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add templates/admin/styles/new.html
git commit -m "feat(styles): live MapLibre spec validation on the new-style page"
```

---

### Task 5: Manual browser verification

**Files:**
- No code changes expected; fixes discovered here are amended into the relevant file with a `fix(styles): ...` commit.

**Interfaces:**
- Consumes: everything from Tasks 1–4, plus real styles in `config/mvtrs.db` (`mapa1` = full style, `politico` = fragment).

- [ ] **Step 1: Run the server**

Run: `cargo run`
Expected: server starts on the configured port (default 5887).

- [ ] **Step 2: Full style validates clean**

In a browser, log into `/admin`, open Styles → edit `mapa1`.
Expected: green "style is valid" message under the editor (translated per locale).

- [ ] **Step 3: Fragment validates clean (no synthetic false positives)**

Open Styles → edit `politico` (fragment, no `version`; some layers may omit `source`).
Expected: green valid message; no errors about `version`, `glyphs`, `sources`, `sprite`, or missing layer `source`.

- [ ] **Step 4: Induced spec error is reported with its path**

In `politico`, change a paint value to a wrong type (e.g. `"fill-color": 42`), wait ~1 s.
Expected: red list appears with an entry whose path points at `layers[...].paint.fill-color`. Undo the change; panel returns to green.

- [ ] **Step 5: Spec errors do not block saving**

With the induced error still present, click Apply.
Expected: the request succeeds (map iframe reloads); no blocking. Restore the style and Apply again.

- [ ] **Step 6: Broken JSON still blocks and hides the lint panel**

Delete a closing brace in the code editor.
Expected: lint panel disappears; clicking Update shows the existing `#jsonError` message and does not submit.

- [ ] **Step 7: Server-side rejection**

With the server running, grab a real category id and post broken JSON:

```bash
CAT_ID=$(sqlite3 config/mvtrs.db "SELECT id FROM categories LIMIT 1;")
curl -s -o /dev/null -w "%{http_code}\n" -X POST http://localhost:5887/admin/styles/create \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data-urlencode "name=test_invalid" \
  --data-urlencode "category=$CAT_ID" \
  --data-urlencode "description=x" \
  --data-urlencode "style={not json"
```

Expected: `400` (or a redirect to login if unauthenticated — in that case verify via the browser dev-tools by tampering the hidden `style` input instead).

- [ ] **Step 8: New-style page behaves**

Open Styles → New. Expected: no panel initially (empty object); clicking "Insert full example" shows green valid; clicking "Insert partial example" shows green valid.

- [ ] **Step 9: CDN-failure degradation**

In dev tools, block requests to `cdn.jsdelivr.net` (or go offline) and reload the edit page.
Expected: editor, Apply and Update all work exactly as before; no lint panel; only a module-load error in the console.

- [ ] **Step 10: Commit any fixes**

If Steps 2–9 surfaced fixes, commit them:

```bash
git add -A
git commit -m "fix(styles): adjustments from manual verification of style linting"
```
