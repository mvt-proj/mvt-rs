// SLD → MapLibre import panel, shared by new.html and edit.html.
//
// Gets a handle on the page's JSONEditor via the same `style-editor-ready`
// CustomEvent that style-validator.js listens for (detail.editor). On
// import, POSTs the file's text content (read client-side, never uploaded
// as a real file) to /admin/styles/convert-sld, then merges the returned
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
  const toggleButton = document.getElementById('btnToggleSldImport');
  const collapse = document.getElementById('sldImportCollapse');
  if (toggleButton && collapse) {
    const icon = toggleButton.querySelector('i');
    toggleButton.addEventListener('click', () => {
      const isHidden = collapse.style.display === 'none';
      collapse.style.display = isHidden ? '' : 'none';
      toggleButton.setAttribute('aria-expanded', String(isHidden));
      if (icon) {
        icon.classList.toggle('fa-chevron-down', !isHidden);
        icon.classList.toggle('fa-chevron-up', isHidden);
      }
    });
  }

  const button = document.getElementById('btnImportSld');
  const importPanel = document.getElementById('sldImportPanel');
  if (!button || !importPanel) {
    return;
  }

  const fileInput = document.getElementById('sldFileInput');
  const sourceLayerSelect = document.getElementById('sldSourceLayer');
  const modeSelect = document.getElementById('sldOutputMode');
  const errorSlot = document.getElementById('sldImportError');
  const warningsPanel = document.getElementById('sldImportWarnings');
  const genericErrorMsg = importPanel.dataset.msgGenericError || 'Could not convert the SLD file';
  const tooLargeErrorMsg = importPanel.dataset.msgTooLarge || genericErrorMsg;
  const noFileErrorMsg = importPanel.dataset.msgNoFile || genericErrorMsg;

  function showError(message) {
    errorSlot.textContent = message;
    errorSlot.style.display = 'block';
  }

  function clearError() {
    errorSlot.style.display = 'none';
  }

  function hideStalePanels() {
    clearError();
    warningsPanel.style.display = 'none';
  }

  // The warnings panel (and, by the same logic, the error panel) must only
  // ever reflect the *latest* conversion: it should appear right after an
  // import and disappear again the moment the editor content is next
  // touched manually (JSONEditor keystroke, or the page's own "load
  // full/partial style" / QML import buttons — anything that dispatches
  // `style-editor-changed`). This listener is the single place that hides
  // both panels. Our own successful-import path below also dispatches
  // `style-editor-changed`, which runs this listener synchronously *before*
  // `renderWarnings()` is called, so the fresh warnings for that import are
  // shown right after being cleared, not clobbered by it.
  document.addEventListener('style-editor-changed', hideStalePanels);

  button.addEventListener('click', async () => {
    hideStalePanels();

    const file = fileInput.files[0];
    if (!file) {
      showError(noFileErrorMsg);
      return;
    }
    if (!editorRef) {
      return;
    }

    let sldText;
    try {
      sldText = await readFileAsText(file);
    } catch (e) {
      showError(genericErrorMsg);
      return;
    }

    const mergeMode = document.querySelector('input[name="sldMergeMode"]:checked').value;

    button.disabled = true;
    try {
      let response;
      try {
        response = await fetch('/admin/styles/convert-sld', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            sld: sldText,
            source_layer: sourceLayerSelect.value,
            mode: modeSelect.value,
          }),
        });
      } catch (e) {
        showError(genericErrorMsg);
        return;
      }

      const contentType = response.headers.get('content-type') || '';
      const isJson = contentType.includes('application/json');

      if (!response.ok) {
        // Validation failures raised by our own handler (bad mode, empty
        // source_layer, malformed SLD, etc.) always come back as JSON with
        // an `error` field, because this endpoint only ever receives
        // fetch's default `Accept: */*` and our error Writer renders JSON
        // unless the client explicitly asked for `text/html`.
        //
        // A non-JSON error body on this endpoint therefore isn't one of
        // those — it's Salvo's own request-parsing layer rejecting the
        // request before our handler ever runs. In practice that almost
        // always means the body was too large for the server's request
        // size limit (oversized SLD file): Salvo silently discards the
        // body in that case, which then surfaces as a plain 400 "missing
        // field" parse error rendered as an HTML page by mvt-rs's default
        // error catcher, not as a 413. Whatever the exact status code, key
        // off "non-JSON body" to give the more actionable hint here.
        let message = isJson ? genericErrorMsg : tooLargeErrorMsg;
        if (isJson) {
          try {
            const errJson = await response.json();
            if (errJson.error) {
              message = errJson.error;
            }
          } catch (e) {
            // Malformed JSON error body: keep the generic message.
          }
        }
        showError(message);
        return;
      }

      // response.ok is also true for a transparently-followed redirect to
      // /login on session expiry (session_auth_handler), whose body is the
      // login page's HTML, not JSON. Guard the parse so that case shows a
      // clear error instead of an unhandled promise rejection.
      let result;
      try {
        if (!isJson) {
          throw new Error('non-JSON response body');
        }
        result = await response.json();
      } catch (e) {
        showError(genericErrorMsg);
        return;
      }

      if (!Array.isArray(result.layers)) {
        showError(genericErrorMsg);
        return;
      }

      let currentJson;
      try {
        currentJson = editorRef.get();
      } catch (e) {
        // The editor currently holds invalid/unparseable JSON (e.g. mid-manual-edit).
        // Abort rather than silently discarding the user's in-progress edits by
        // merging into an empty object.
        showError(genericErrorMsg);
        return;
      }

      editorRef.set(mergeLayers(currentJson, result.layers, mergeMode));
      document.dispatchEvent(new Event('style-editor-changed'));
      renderWarnings(warningsPanel, result.warnings);
    } finally {
      button.disabled = false;
    }
  });
});
