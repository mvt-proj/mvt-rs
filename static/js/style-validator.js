// Live MapLibre style-spec validation for the admin style editor.
//
// Self-wiring: the editor page dispatches `style-editor-ready` (CustomEvent
// with detail.editor = JSONEditor instance) once, and `style-editor-changed`
// on every edit. This module listens, validates (debounced) and renders into
// #styleLintPanel. Spec errors only warn — saving is never blocked here.
// If the CDN import fails the module never executes and the editor keeps
// working without lint feedback.

import { validateStyleMin, latest } from 'https://cdn.jsdelivr.net/npm/@maplibre/maplibre-gl-style-spec@25.0.2/dist/index.mjs';

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
// Only the "layers" key is validated; other fragment keys are intentionally
// ignored (the mvt-rs fragment convention is layers-only).
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

// validateStyleMin deliberately ignores unknown properties at the style root,
// at the layer level, and inside vector/raster sources (catch-all validators,
// so the GL runtime's leniency toward vendor extensions carries over; other
// source types, paint and layout are strict upstream). For typo detection in
// the editor we want those flagged, so this sweeps the user's original JSON
// against the spec's own key lists.
const LENIENT_SOURCE_SPECS = {
  vector: 'source_vector',
  raster: 'source_raster',
};

function unknownKeyErrors(json) {
  const errors = [];
  if ('version' in json) {
    for (const key of Object.keys(json)) {
      if (!(key in latest.$root)) {
        errors.push({ path: key, message: `unknown property "${key}"` });
      }
    }
  }
  if (json.sources && typeof json.sources === 'object') {
    for (const [name, source] of Object.entries(json.sources)) {
      if (!source || typeof source !== 'object') continue;
      const specKey = LENIENT_SOURCE_SPECS[source.type];
      if (!specKey) continue;
      for (const key of Object.keys(source)) {
        if (!(key in latest[specKey])) {
          errors.push({ path: `sources.${name}.${key}`, message: `unknown property "${key}"` });
        }
      }
    }
  }
  if (Array.isArray(json.layers)) {
    json.layers.forEach((layer, i) => {
      if (!layer || typeof layer !== 'object') return;
      for (const key of Object.keys(layer)) {
        // "ref" is deprecated and gone from the spec, but the validator and
        // runtime still accept it.
        if (!(key in latest.layer) && key !== 'ref') {
          errors.push({ path: `layers[${i}].${key}`, message: `unknown property "${key}"` });
        }
      }
    });
  }
  return errors;
}

export function validateStyle(json) {
  try {
    if (json && typeof json === 'object' && 'version' in json) {
      return validateStyleMin(json).map(toDisplayError).concat(unknownKeyErrors(json));
    }
    const wrapped = wrapFragment(json ?? {});
    if (wrapped === null) {
      return [{ path: 'layers', message: 'a partial style must contain a "layers" array' }];
    }
    return validateStyleMin(wrapped)
      .map(toDisplayError)
      .filter((e) => !e.message.includes(DUMMY_SOURCE) && !e.path.includes(DUMMY_SOURCE))
      .concat(unknownKeyErrors(json));
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
  if (errors.length > 50) {
    const more = document.createElement('li');
    more.textContent = `… +${errors.length - 50}`;
    list.appendChild(more);
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
