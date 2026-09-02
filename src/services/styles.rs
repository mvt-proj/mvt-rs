use crate::{
    error::AppResult,
    models::styles::Style,
    services::{mvt_url::rewrite_mvt_tokens, tilejson::base_url_from_request},
};
use salvo::prelude::*;
use serde_json::Value;

/// Parses a stored style document and rewrites any `mvt://` placeholder
/// tokens (`sources[].tiles`, `sprite`, `glyphs`) into absolute URLs rooted
/// at `base_url` — the environment actually answering the request.
///
/// `pub(crate)` so `html::maps::page_map_view` can reuse it for the style
/// preview page, which embeds the style document directly instead of
/// fetching it from this endpoint.
pub(crate) fn resolve_style_json(style_json: &str, base_url: &str) -> AppResult<Value> {
    let mut value: Value = serde_json::from_str(style_json)?;
    rewrite_mvt_tokens(&mut value, base_url);
    Ok(value)
}

#[handler]
pub async fn index(req: &mut Request, _res: &mut Response) -> AppResult<Json<Value>> {
    let style_name = req.param::<String>("style_name").unwrap_or("".to_string());
    let parts: Vec<&str> = style_name.split(':').collect();

    let category = parts.first().unwrap_or(&"");
    let name = parts.get(1).unwrap_or(&"");
    let style = Style::from_category_and_name_cached(category, name).await?;

    let base_url = base_url_from_request(req);
    Ok(Json(resolve_style_json(&style.style, &base_url)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_style_json_rewrites_mvt_tokens_in_sources_sprite_and_glyphs() {
        let style_json = r#"{
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
        }"#;

        let result = resolve_style_json(style_json, "https://mvt.example.com").unwrap();

        assert_eq!(
            result["sources"]["parcels"]["tiles"][0],
            "https://mvt.example.com/services/tiles/public:parcels/{z}/{x}/{y}.pbf"
        );
        assert_eq!(
            result["sources"]["arrendamientos"]["tiles"][0],
            "https://mvt.example.com/services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf"
        );
        assert_eq!(
            result["sprite"],
            "https://mvt.example.com/services/map_assets/sprites/fa-brand/sprite"
        );
        assert_eq!(
            result["glyphs"],
            "https://mvt.example.com/services/map_assets/glyphs/{fontstack}/{range}.pbf"
        );
    }

    #[test]
    fn resolve_style_json_leaves_already_resolved_urls_untouched() {
        let style_json = r#"{"sprite": "https://cdn.example.com/sprite"}"#;
        let result = resolve_style_json(style_json, "https://mvt.example.com").unwrap();
        assert_eq!(result["sprite"], "https://cdn.example.com/sprite");
    }
}
