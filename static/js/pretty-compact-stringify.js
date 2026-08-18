// Vendored from json-stringify-pretty-compact v4.0.0 (MIT License)
// https://github.com/lydell/json-stringify-pretty-compact
//
// Like JSON.stringify(obj, null, indent), but keeps arrays/objects that fit
// within `maxLength` on a single line instead of always expanding them.
// Used to keep short MapLibre style expressions (e.g. ["get", "type"])
// readable on one line while still indenting the rest of the document.
(function (global) {
  const stringOrChar = /("(?:[^\\"]|\\.)*")|[:,]/g;

  function prettyCompactStringify(passedObj, options) {
    options = options || {};

    const indent = JSON.stringify(
      [1],
      undefined,
      options.indent === undefined ? 2 : options.indent
    ).slice(2, -3);

    const maxLength =
      indent === ""
        ? Infinity
        : options.maxLength === undefined
        ? 80
        : options.maxLength;

    let replacer = options.replacer;

    return (function _stringify(obj, currentIndent, reserved) {
      if (obj && typeof obj.toJSON === "function") {
        obj = obj.toJSON();
      }

      const string = JSON.stringify(obj, replacer);

      if (string === undefined) {
        return string;
      }

      const length = maxLength - currentIndent.length - reserved;

      if (string.length <= length) {
        const prettified = string.replace(stringOrChar, (match, stringLiteral) => {
          return stringLiteral || `${match} `;
        });
        if (prettified.length <= length) {
          return prettified;
        }
      }

      if (replacer != null) {
        obj = JSON.parse(string);
        replacer = undefined;
      }

      if (typeof obj === "object" && obj !== null) {
        const nextIndent = currentIndent + indent;
        const items = [];
        let index = 0;
        let start;
        let end;

        if (Array.isArray(obj)) {
          start = "[";
          end = "]";
          const length = obj.length;
          for (; index < length; index++) {
            items.push(
              _stringify(obj[index], nextIndent, index === length - 1 ? 0 : 1) || "null"
            );
          }
        } else {
          start = "{";
          end = "}";
          const keys = Object.keys(obj);
          const length = keys.length;
          for (; index < length; index++) {
            const key = keys[index];
            const keyPart = `${JSON.stringify(key)}: `;
            const value = _stringify(
              obj[key],
              nextIndent,
              keyPart.length + (index === length - 1 ? 0 : 1)
            );
            if (value !== undefined) {
              items.push(keyPart + value);
            }
          }
        }

        if (items.length > 0) {
          return [start, indent + items.join(`,\n${nextIndent}`), end].join(`\n${currentIndent}`);
        }
      }

      return string;
    })(passedObj, "", 0);
  }

  // Patches a jsoneditor (josdejong) instance so that, while in 'code' or
  // 'text' mode, its Format button and editor.set() use prettyCompactStringify
  // instead of the library's default JSON.stringify(obj, null, indentation).
  // jsoneditor re-mixes .format/.set into the instance on every mode switch,
  // so this must be re-applied (e.g. from the `onModeChange` option) each
  // time the mode becomes 'code' or 'text'.
  function applyCompactJsonEditorFormat(editor) {
    if (typeof editor.format !== "function" || typeof editor.setText !== "function") {
      return;
    }

    const indentation = typeof editor.indentation === "number" ? editor.indentation : 2;

    editor.format = function () {
      const json = this.get();
      this.updateText(prettyCompactStringify(json, { indent: indentation }));
    };

    editor.set = function (json) {
      this.setText(prettyCompactStringify(json, { indent: indentation }));
    };
  }

  global.prettyCompactStringify = prettyCompactStringify;
  global.applyCompactJsonEditorFormat = applyCompactJsonEditorFormat;
})(window);
