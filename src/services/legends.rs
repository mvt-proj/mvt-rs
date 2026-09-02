use crate::{
    error::AppResult,
    models::styles::Style,
    services::{styles::resolve_style_json, tilejson::base_url_from_request},
};
use maplibre_legend::{LegendConfig, MapLibreLegend};
use salvo::prelude::*;

#[handler]
pub async fn index(req: &mut Request, res: &mut Response) -> AppResult<()> {
    res.headers_mut()
        .insert("content-type", "image/svg+xml".parse()?);

    let style_name = req.param::<String>("style_name").unwrap_or_default();
    let layer_id = req.query::<String>("layer_id").unwrap_or_default();
    let default_width = req.query::<u32>("width").unwrap_or(250);
    let default_height = req.query::<u32>("height").unwrap_or(40);
    let has_label = req.query::<bool>("has_label").unwrap_or_default();
    let include_raster = req.query::<bool>("include_raster").unwrap_or_default();
    let reverse = req.query::<bool>("reverse").unwrap_or_default();
    let parts: Vec<&str> = style_name.split(':').collect();

    let category = parts.first().unwrap_or(&"");
    let name = parts.get(1).unwrap_or(&"");
    let style = Style::from_category_and_name_cached(category, name).await?;

    // The legend renderer fetches sprite/glyph assets itself (server-side),
    // so it needs real, fetchable URLs — never the stored `mvt://` tokens.
    let base_url = base_url_from_request(req);
    let resolved_style_json = resolve_style_json(&style.style, &base_url)?.to_string();

    let legend = MapLibreLegend::new(
        &resolved_style_json,
        LegendConfig {
            default_width,
            default_height,
            has_label,
            include_raster,
        },
    )
    .await?;

    if !layer_id.is_empty() {
        let svg = legend.render_layer(&layer_id, Some(has_label))?;
        res.render(svg);
    } else {
        let svg = legend.render_all(reverse)?;
        res.render(svg);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test: the legend renderer fetches sprite/glyph assets
    /// itself, server-side — passing it the raw stored style (with
    /// unresolved `mvt://` tokens) makes it try to HTTP-fetch a
    /// non-`http(s)` URL and fail with a builder/status-500 error, the
    /// same failure mode the mapview preview had before it went through
    /// `resolve_style_json` too.
    #[test]
    fn legend_input_has_mvt_tokens_resolved_before_reaching_maplibre_legend() {
        let style_json = r#"{
            "version": 8,
            "sprite": "mvt://services/map_assets/sprites/maplibre/sprite",
            "sources": {}
        }"#;

        let resolved = resolve_style_json(style_json, "http://127.0.0.1:5887")
            .unwrap()
            .to_string();

        assert!(
            !resolved.contains("mvt://"),
            "legend input must not contain raw mvt:// tokens: {resolved}"
        );
        assert!(
            resolved.contains("http://127.0.0.1:5887/services/map_assets/sprites/maplibre/sprite"),
            "expected the resolved sprite URL in {resolved}"
        );
    }
}
