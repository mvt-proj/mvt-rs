// mvt_url.rs
//
// `mvt://` is a non-resolvable placeholder scheme stored in style JSON
// (`sources[].tiles`, `sprite`, `glyphs`) instead of a real, environment-bound
// URL. It is rewritten to an absolute URL only when a style is served, using
// whichever environment answers the request — so promoting config (full DB
// copy or selective export) across environments never carries a baked-in
// host along with it.

use serde_json::Value;

pub const MVT_SCHEME: &str = "mvt://";

/// Resolves a single `mvt://` token into an absolute URL rooted at `base_url`.
/// A token is `mvt://` followed by whatever comes after `scheme://host[:port]`
/// in the real URL (e.g. `mvt://services/tiles/category/parcels/{z}/{x}/{y}.pbf`)
/// — no knowledge of which endpoint shape it is, so any current or future
/// path under the server's origin works without touching this function.
/// Returns `None` if `value` is not an `mvt://` token.
pub fn resolve_mvt_token(value: &str, base_url: &str) -> Option<String> {
    let rest = value.strip_prefix(MVT_SCHEME)?;
    let base_url = base_url.trim_end_matches('/');
    Some(format!("{base_url}/{rest}"))
}

/// Recursively walks a JSON value, rewriting every `mvt://` string found
/// anywhere in it (e.g. `sources[].tiles`, top-level `sprite`/`glyphs` in a
/// MapLibre style document) into an absolute URL rooted at `base_url`.
pub fn rewrite_mvt_tokens(value: &mut Value, base_url: &str) {
    match value {
        Value::String(s) => {
            if let Some(resolved) = resolve_mvt_token(s, base_url) {
                *s = resolved;
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_mvt_tokens(item, base_url);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                rewrite_mvt_tokens(v, base_url);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_mvt_token_rewrites_single_layer_tile_source() {
        let resolved = resolve_mvt_token(
            "mvt://services/tiles/public:parcels/{z}/{x}/{y}.pbf",
            "https://mvt.example.com",
        );
        assert_eq!(
            resolved,
            Some("https://mvt.example.com/services/tiles/public:parcels/{z}/{x}/{y}.pbf".to_string())
        );
    }

    #[test]
    fn resolve_mvt_token_rewrites_category_wide_tile_source() {
        let resolved = resolve_mvt_token(
            "mvt://services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf",
            "https://mvt.example.com",
        );
        assert_eq!(
            resolved,
            Some(
                "https://mvt.example.com/services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf"
                    .to_string()
            )
        );
    }

    #[test]
    fn resolve_mvt_token_rewrites_multi_layer_tile_source() {
        let resolved = resolve_mvt_token(
            "mvt://services/tiles/multi/public:parcels,public:roads/{z}/{x}/{y}.pbf",
            "https://mvt.example.com",
        );
        assert_eq!(
            resolved,
            Some(
                "https://mvt.example.com/services/tiles/multi/public:parcels,public:roads/{z}/{x}/{y}.pbf"
                    .to_string()
            )
        );
    }

    #[test]
    fn resolve_mvt_token_rewrites_sprite() {
        let resolved = resolve_mvt_token(
            "mvt://services/map_assets/sprites/fa-brand/sprite",
            "https://mvt.example.com",
        );
        assert_eq!(
            resolved,
            Some("https://mvt.example.com/services/map_assets/sprites/fa-brand/sprite".to_string())
        );
    }

    #[test]
    fn resolve_mvt_token_rewrites_glyphs() {
        let resolved = resolve_mvt_token(
            "mvt://services/map_assets/glyphs/{fontstack}/{range}.pbf",
            "https://mvt.example.com",
        );
        assert_eq!(
            resolved,
            Some("https://mvt.example.com/services/map_assets/glyphs/{fontstack}/{range}.pbf".to_string())
        );
    }

    #[test]
    fn resolve_mvt_token_strips_trailing_slash_from_base_url() {
        let resolved = resolve_mvt_token(
            "mvt://services/map_assets/sprites/fa-brand/sprite",
            "https://mvt.example.com/",
        );
        assert_eq!(
            resolved,
            Some("https://mvt.example.com/services/map_assets/sprites/fa-brand/sprite".to_string())
        );
    }

    #[test]
    fn resolve_mvt_token_returns_none_for_non_token_value() {
        assert_eq!(resolve_mvt_token("https://mvt-dev.example.com/services/tiles/public:parcels/{z}/{x}/{y}.pbf", "https://mvt.example.com"), None);
    }

    #[test]
    fn rewrite_mvt_tokens_walks_nested_style_json() {
        let mut style = json!({
            "version": 8,
            "sources": {
                "parcels": {
                    "type": "vector",
                    "tiles": ["mvt://services/tiles/public:parcels/{z}/{x}/{y}.pbf"]
                },
                "arrendamientos": {
                    "type": "vector",
                    "tiles": ["mvt://services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf"]
                }
            },
            "sprite": "mvt://services/map_assets/sprites/fa-brand/sprite",
            "glyphs": "mvt://services/map_assets/glyphs/{fontstack}/{range}.pbf"
        });

        rewrite_mvt_tokens(&mut style, "https://mvt.example.com");

        assert_eq!(
            style["sources"]["parcels"]["tiles"][0],
            "https://mvt.example.com/services/tiles/public:parcels/{z}/{x}/{y}.pbf"
        );
        assert_eq!(
            style["sources"]["arrendamientos"]["tiles"][0],
            "https://mvt.example.com/services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf"
        );
        assert_eq!(
            style["sprite"],
            "https://mvt.example.com/services/map_assets/sprites/fa-brand/sprite"
        );
        assert_eq!(
            style["glyphs"],
            "https://mvt.example.com/services/map_assets/glyphs/{fontstack}/{range}.pbf"
        );
    }

    #[test]
    fn rewrite_mvt_tokens_leaves_non_token_strings_untouched() {
        let mut style = json!({
            "name": "Base style",
            "sprite": "https://cdn.example.com/sprite"
        });

        rewrite_mvt_tokens(&mut style, "https://mvt.example.com");

        assert_eq!(style["name"], "Base style");
        assert_eq!(style["sprite"], "https://cdn.example.com/sprite");
    }
}
