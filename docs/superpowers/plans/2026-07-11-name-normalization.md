# Admin Name Field Normalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Names of catalog layers, styles, categories and groups are always normalized to `[a-z0-9_]` (lowercase, spaces→`_`, accents transliterated), enforced server-side and shown live in the admin forms.

**Architecture:** A single pure function `normalize_name()` in `src/services/utils.rs` is the source of truth. It is applied inside the model mutation methods (`Group::new`/`update_group`, `Category::new`/`update_category`, `Style::new`/`update_style`, `Catalog::add_layer`/`update_layer`) so both the HTML admin forms and the JSON API (`POST /api/catalog/layer`) are covered. A mirrored JS implementation in the shared admin layout normalizes as-you-type on any `input[data-normalize-name]`.

**Tech Stack:** Rust (Salvo 0.93, SQLx), Askama templates, vanilla JS.

**Spec:** `docs/superpowers/specs/2026-07-11-name-normalization-design.md`

## Global Constraints

- Normalization rule (identical in Rust and JS): trim ends → lowercase → transliterate `á à ä â é è ë ê í ì ï î ó ò ö ô ú ù ü û ñ` → `a e i o u n` equivalents → whitespace runs → `_` → drop any char not in `[a-z0-9_]` → collapse repeated `_`.
- Server-side: empty result after normalization → `Err(AppError::InvalidInput(...))` (maps to HTTP 400).
- JS live version does NOT strip leading/trailing `_` (you couldn't type a separator otherwise); the server does the final cleanup.
- Users entity is out of scope. No migration of existing stored names.
- Every task: run `cargo test` before committing; all tests must pass.

---

### Task 1: `normalize_name()` in Rust (TDD)

**Files:**
- Modify: `src/services/utils.rs` (add function at end of file)
- Test: `src/services/tests.rs` (add tests inside existing `mod tests`)

**Interfaces:**
- Produces: `pub fn normalize_name(name: &str) -> AppResult<String>` in `crate::services::utils`. Later tasks (2, 3) call it.

- [ ] **Step 1: Write the failing tests**

Add inside the existing `mod tests { ... }` block in `src/services/tests.rs` (note: the `use` goes next to the existing `use crate::services::utils::validate_filter;`):

```rust
    use crate::services::utils::normalize_name;

    #[test]
    fn test_normalize_name_spaces_and_case() {
        assert_eq!(
            normalize_name("departamentos Capital").unwrap(),
            "departamentos_capital"
        );
        assert_eq!(normalize_name("GRUPOS").unwrap(), "grupos");
    }

    #[test]
    fn test_normalize_name_accents() {
        assert_eq!(normalize_name("Categoría Ríos").unwrap(), "categoria_rios");
        assert_eq!(normalize_name("Ñandú Güemes").unwrap(), "nandu_guemes");
    }

    #[test]
    fn test_normalize_name_collapses_separators() {
        assert_eq!(normalize_name("  foo   bar  ").unwrap(), "foo_bar");
        assert_eq!(normalize_name("foo__bar").unwrap(), "foo_bar");
        assert_eq!(normalize_name("foo \t bar").unwrap(), "foo_bar");
    }

    #[test]
    fn test_normalize_name_drops_symbols() {
        assert_eq!(normalize_name("depto. (norte)").unwrap(), "depto_norte");
        assert_eq!(normalize_name("capa-2024!").unwrap(), "capa2024");
    }

    #[test]
    fn test_normalize_name_no_leading_or_trailing_underscore() {
        assert_eq!(normalize_name("!foo bar!").unwrap(), "foo_bar");
        assert_eq!(normalize_name(" _foo_ ").unwrap(), "foo");
    }

    #[test]
    fn test_normalize_name_already_normalized_passthrough() {
        assert_eq!(normalize_name("departamentos_capital").unwrap(), "departamentos_capital");
    }

    #[test]
    fn test_normalize_name_empty_result_is_error() {
        assert!(normalize_name("").is_err());
        assert!(normalize_name("   ").is_err());
        assert!(normalize_name("!!!").is_err());
        assert!(normalize_name("___").is_err());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test normalize_name`
Expected: compile error `cannot find function normalize_name in module crate::services::utils` (a missing function is the Rust equivalent of a failing test at this stage).

- [ ] **Step 3: Implement `normalize_name`**

Append at the end of `src/services/utils.rs` (the file already imports `AppError` and `AppResult` from `crate::error`; if only `AppResult` is imported, extend that `use` to include `AppError`):

```rust
/// Normalizes an entity name (layer, style, category, group) for use in URLs
/// and cache keys: lowercase, accents transliterated, whitespace runs become
/// `_`, anything outside `[a-z0-9_]` is dropped, repeated `_` collapsed.
/// Errors if nothing valid remains.
pub fn normalize_name(name: &str) -> AppResult<String> {
    let mut result = String::with_capacity(name.len());
    for ch in name.trim().to_lowercase().chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            ' ' | '\t' | '_' => Some('_'),
            'á' | 'à' | 'ä' | 'â' => Some('a'),
            'é' | 'è' | 'ë' | 'ê' => Some('e'),
            'í' | 'ì' | 'ï' | 'î' => Some('i'),
            'ó' | 'ò' | 'ö' | 'ô' => Some('o'),
            'ú' | 'ù' | 'ü' | 'û' => Some('u'),
            'ñ' => Some('n'),
            _ => None,
        };
        if let Some(c) = mapped {
            if c == '_' {
                if !result.is_empty() && !result.ends_with('_') {
                    result.push('_');
                }
            } else {
                result.push(c);
            }
        }
    }
    let result = result.trim_end_matches('_');
    if result.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "name '{name}' contains no valid characters (allowed: a-z, 0-9, _)"
        )));
    }
    Ok(result.to_string())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test normalize_name`
Expected: 7 tests PASS.

Run: `cargo test`
Expected: full suite PASS (no regressions).

- [ ] **Step 5: Commit**

```bash
git add src/services/utils.rs src/services/tests.rs
git commit -m "feat: add normalize_name helper for entity names"
```

---

### Task 2: Apply normalization in Group, Category and Style models

**Files:**
- Modify: `src/auth/models.rs` (`Group::new` ~line 41, `Group::update_group` ~line 64)
- Modify: `src/models/category.rs` (`Category::new` ~line 17, `Category::update_category` ~line 39)
- Modify: `src/models/styles.rs` (`Style::new` ~line 22, `Style::update_style` ~line 77)

**Interfaces:**
- Consumes: `crate::services::utils::normalize_name(name: &str) -> AppResult<String>` from Task 1.
- Produces: no new interfaces — same method signatures, names are normalized before persisting.

There are no unit tests for these methods (they hit SQLite + global state); the pure logic is covered by Task 1's tests and the full flow by Task 5's manual verification. Verification here is `cargo test` (no regressions).

- [ ] **Step 1: Normalize in `Group::new` and `Group::update_group`**

In `src/auth/models.rs`, add `let name = crate::services::utils::normalize_name(&name)?;` as the FIRST line of both method bodies. `Group::new` becomes:

```rust
    pub async fn new(name: String, description: String) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let group = Group {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
        };
        // ... rest unchanged
```

and `update_group` becomes:

```rust
    pub async fn update_group(&self, name: String, description: String) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let group = Group {
            id: self.id.clone(),
            name,
            description,
        };
        // ... rest unchanged
```

(Use the fully qualified path `crate::services::utils::normalize_name` — this file's `use` block is large; no import edit needed.)

- [ ] **Step 2: Normalize in `Category::new` and `Category::update_category`**

Same pattern in `src/models/category.rs` — first line of both bodies:

```rust
    pub async fn new(name: String, description: String) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let category = Category {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
        };
        // ... rest unchanged
```

```rust
    pub async fn update_category(&self, name: String, description: String) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let category = Category {
            id: self.id.clone(),
            name,
            description,
        };
        // ... rest unchanged
```

- [ ] **Step 3: Normalize in `Style::new` and `Style::update_style`**

Same pattern in `src/models/styles.rs` — first line of both bodies (note the local variable shadowing: the parameter is `name: String`, the struct field assignment `name,` stays unchanged):

```rust
    pub async fn new(
        name: String,
        category: Category,
        description: String,
        style: String,
    ) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let style = Style {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            category,
            description,
            style,
        };
        // ... rest unchanged
```

```rust
    pub async fn update_style(
        &self,
        name: String,
        category: Category,
        description: String,
        style: String,
    ) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let style = Style {
            id: self.id.clone(),
            name,
            category,
            description,
            style,
        };
        // ... rest unchanged
```

- [ ] **Step 4: Verify build and tests**

Run: `cargo test`
Expected: full suite PASS.

- [ ] **Step 5: Commit**

```bash
git add src/auth/models.rs src/models/category.rs src/models/styles.rs
git commit -m "feat: normalize group, category and style names on create/update"
```

---

### Task 3: Apply normalization to catalog layers (+ cache key alignment)

**Files:**
- Modify: `src/models/catalog.rs` (`Catalog::add_layer` ~line 313, `Catalog::update_layer` ~line 319)
- Modify: `src/html/admin/catalog.rs` (`layer_key` computation ~line 232)

**Interfaces:**
- Consumes: `crate::services::utils::normalize_name(name: &str) -> AppResult<String>` from Task 1.
- Produces: no new interfaces.

**Why the handler edit:** the HTML `update_layer` handler computes the tile-cache invalidation key from the raw form value (`format!("{}_{}", category.name, layer_form.name)`) BEFORE the model normalizes the stored name. Without aligning it, editing a layer typed as `"Mi Capa"` would clear cache key `categoria_Mi Capa` while tiles are served under `categoria_mi_capa`.

- [ ] **Step 1: Normalize in `Catalog::add_layer` and `Catalog::update_layer`**

In `src/models/catalog.rs`, change both methods to take `mut layer` and normalize the name first. `add_layer` (covers the HTML form AND `POST /api/catalog/layer`):

```rust
    pub async fn add_layer(&mut self, mut layer: Layer) -> AppResult<()> {
        layer.name = crate::services::utils::normalize_name(&layer.name)?;
        create_layer(None, layer.clone()).await?;
        self.layers.push(layer);
        Ok(())
    }
```

`update_layer`:

```rust
    pub async fn update_layer(&mut self, mut layer: Layer) -> AppResult<()> {
        layer.name = crate::services::utils::normalize_name(&layer.name)?;
        update_layer(None, layer.clone()).await?;
        let position = self.layers.iter().position(|lyr| lyr.id == layer.id);
        match position {
            Some(index) => self.layers[index] = layer,
            None => println!("layer not found"),
        }

        Ok(())
    }
```

- [ ] **Step 2: Align the cache invalidation key in the HTML handler**

In `src/html/admin/catalog.rs` (`update_layer` handler, ~line 232), change:

```rust
    let layer_key = format!("{}_{}", category.name, layer_form.name);
```

to:

```rust
    let layer_key = format!(
        "{}_{}",
        category.name,
        crate::services::utils::normalize_name(&layer_form.name)?
    );
```

- [ ] **Step 3: Verify build and tests**

Run: `cargo test`
Expected: full suite PASS.

- [ ] **Step 4: Commit**

```bash
git add src/models/catalog.rs src/html/admin/catalog.rs
git commit -m "feat: normalize layer names on create/update and align cache key"
```

---

### Task 4: Live normalization in admin forms (JS + template attributes)

**Files:**
- Modify: `templates/admin/layout_admin.html` (shared `<script>` block, lines 28–44)
- Modify: `templates/admin/categories/new.html` (~line 18)
- Modify: `templates/admin/categories/edit.html` (~lines 31–38)
- Modify: `templates/admin/groups/new.html` (~line 18)
- Modify: `templates/admin/groups/edit.html` (~lines 31–38)
- Modify: `templates/admin/styles/new.html` (~line 45)
- Modify: `templates/admin/styles/edit.html` (~line 58)
- Modify: `templates/admin/catalog/layers/new.html` (~line 49)
- Modify: `templates/admin/catalog/layers/edit.html` (~line 51)

**Interfaces:**
- Consumes: nothing from other tasks (JS mirrors the Task 1 rule independently).
- Produces: any `input[data-normalize-name]` in the admin gets live normalization.

- [ ] **Step 1: Add the JS helper to the shared admin layout**

In `templates/admin/layout_admin.html`, inside the existing IIFE (`(function () { ... })();`), add after the `keydown` listener (before the closing `})();`):

```javascript
  // Live normalization for entity name inputs (mirrors normalize_name() in
  // src/services/utils.rs; the server strips leading/trailing "_" on save).
  var NAME_ACCENTS = {
    'á': 'a', 'à': 'a', 'ä': 'a', 'â': 'a',
    'é': 'e', 'è': 'e', 'ë': 'e', 'ê': 'e',
    'í': 'i', 'ì': 'i', 'ï': 'i', 'î': 'i',
    'ó': 'o', 'ò': 'o', 'ö': 'o', 'ô': 'o',
    'ú': 'u', 'ù': 'u', 'ü': 'u', 'û': 'u',
    'ñ': 'n'
  };

  function normalizeName(value) {
    return value
      .toLowerCase()
      .replace(/[áàäâéèëêíìïîóòöôúùüûñ]/g, function (ch) { return NAME_ACCENTS[ch]; })
      .replace(/\s+/g, '_')
      .replace(/[^a-z0-9_]/g, '')
      .replace(/_+/g, '_');
  }

  document.addEventListener('input', function (e) {
    var el = e.target;
    if (!el.matches || !el.matches('input[data-normalize-name]')) return;
    var before = el.value;
    var after = normalizeName(before);
    if (after === before) return;
    var pos = el.selectionStart - (before.length - after.length);
    el.value = after;
    el.setSelectionRange(Math.max(0, pos), Math.max(0, pos));
  });
```

- [ ] **Step 2: Tag the 8 name inputs**

Add `data-normalize-name` and `pattern="[a-z0-9_]+"` (no-JS fallback) to each input. Exact edits:

`templates/admin/categories/new.html` (~line 18) — from:
```html
          <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" required />
```
to:
```html
          <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" data-normalize-name pattern="[a-z0-9_]+" required />
```

`templates/admin/groups/new.html` (~line 18) — identical change to the line:
```html
          <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" required />
```

`templates/admin/categories/edit.html` (~lines 31–38) — the input is multi-line; from:
```html
          <input
            class="input"
            name="name"
            type="text"
            placeholder="{{ base.translate["name"] }}"
            value="{{ category.name }}"
            required
          />
```
to:
```html
          <input
            class="input"
            name="name"
            type="text"
            placeholder="{{ base.translate["name"] }}"
            value="{{ category.name }}"
            data-normalize-name
            pattern="[a-z0-9_]+"
            required
          />
```

`templates/admin/groups/edit.html` (~lines 31–38) — same multi-line pattern, with `value="{{ group.name }}"`; add the same two attribute lines before `required`.

`templates/admin/styles/new.html` (~line 45) — from:
```html
          <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" required>
```
to:
```html
          <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" data-normalize-name pattern="[a-z0-9_]+" required>
```

`templates/admin/styles/edit.html` (~line 58) — from:
```html
              <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" value="{{ style.name }}" required>
```
to:
```html
              <input class="input" name="name" type="text" placeholder="{{ base.translate["name"] }}" value="{{ style.name }}" data-normalize-name pattern="[a-z0-9_]+" required>
```

`templates/admin/catalog/layers/new.html` (~line 49) — from:
```html
          <input class="input" type="text" name="name" id="name" required />
```
to:
```html
          <input class="input" type="text" name="name" id="name" data-normalize-name pattern="[a-z0-9_]+" required />
```

`templates/admin/catalog/layers/edit.html` (~line 51) — from:
```html
            <input class="input" type="text" name="name" id="name" value="{{ layer.name }}" required>
```
to:
```html
            <input class="input" type="text" name="name" id="name" value="{{ layer.name }}" data-normalize-name pattern="[a-z0-9_]+" required>
```

- [ ] **Step 3: Verify templates compile**

Askama compiles templates into the binary, so:

Run: `cargo check`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add templates/admin/layout_admin.html templates/admin/categories/ templates/admin/groups/ templates/admin/styles/ templates/admin/catalog/layers/
git commit -m "feat: live name normalization in admin forms"
```

---

### Task 5: End-to-end verification

**Files:** none (verification only).

- [ ] **Step 1: Full test suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 2: Manual verification against a running server**

Start the server (`cargo run`, needs the local PostGIS per `docker-compose up -d` if not running), log into the admin and:

1. Create a category named `Categoría  De Prueba` → after saving, the list shows `categoria_de_prueba`.
2. While typing in the name field, spaces appear as `_` and uppercase becomes lowercase live.
3. Create a group named `!!!` → the browser blocks submit (pattern); with JS the invalid chars never appear. Direct POST with an all-invalid name returns HTTP 400.
4. Delete the test category and group.

If a browser isn't available in this session, report the steps as pending manual verification by the user instead of skipping silently.

- [ ] **Step 3: Final commit check**

Run: `git status`
Expected: clean tree (everything committed in Tasks 1–4).
