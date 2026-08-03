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
      // The editor currently holds invalid/unparseable JSON (e.g. mid-manual-edit).
      // Abort rather than silently discarding the user's in-progress edits by
      // merging into an empty object.
      showError(genericErrorMsg);
      return;
    }

    editorRef.set(mergeLayers(currentJson, result.layers, mergeMode));
    document.dispatchEvent(new Event('style-editor-changed'));
    renderWarnings(warningsPanel, result.warnings);
  });
});
