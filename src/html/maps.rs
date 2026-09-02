use super::utils::{BaseTemplateData, make_base};
use crate::db::metadata::{Extent, query_extent};
use crate::get_catalog;
use crate::models::catalog::{Layer, StateLayer};
use crate::models::styles::Style;
use crate::services::{styles::resolve_style_json, tilejson::base_url_from_request};
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "maplayer.html")]
struct MapLayerTemplate<'a> {
    geometry: &'a str,
    layer: Layer,
    extent: Extent,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "maplayer_minimal.html")]
struct MapLayerMinimalTemplate<'a> {
    geometry: &'a str,
    layer: Layer,
    extent: Extent,
    base: BaseTemplateData,
}

enum MapLayerTemplateKind<'a> {
    Minimal(MapLayerMinimalTemplate<'a>),
    Full(MapLayerTemplate<'a>),
}

impl MapLayerTemplateKind<'_> {
    fn render(&self) -> Result<String, askama::Error> {
        match self {
            MapLayerTemplateKind::Minimal(tpl) => tpl.render(),
            MapLayerTemplateKind::Full(tpl) => tpl.render(),
        }
    }
}

#[derive(Template)]
#[template(path = "mapview.html")]
struct MapViewTemplate {
    base: BaseTemplateData,
    style: Style,
    resolved_style_json: String,
}

#[derive(Template)]
#[template(path = "mapview_minimal.html")]
struct MapViewMinimalTemplate {
    base: BaseTemplateData,
    style: Style,
    resolved_style_json: String,
}

enum MapTemplate {
    Minimal(MapViewMinimalTemplate),
    Full(MapViewTemplate),
}

impl MapTemplate {
    fn render(&self) -> Result<String, askama::Error> {
        match self {
            MapTemplate::Minimal(tpl) => tpl.render(),
            MapTemplate::Full(tpl) => tpl.render(),
        }
    }
}

#[handler]
pub async fn page_map_layer(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> Result<(), StatusError> {
    let layer_name = req
        .param::<String>("layer_name")
        .ok_or_else(|| StatusError::bad_request().brief("Missing layer_name parameter"))?;
    let is_minimal = req.query::<bool>("minimal").unwrap_or_default();
    let parts: Vec<&str> = layer_name.split(':').collect();
    let category = parts.first().unwrap_or(&"").to_string();
    let name = parts.get(1).unwrap_or(&"").to_string();

    let (base, _) = make_base(depot).await;

    let (lyr, geometry) = {
        let catalog = get_catalog().await.read().await;
        let lyr = catalog
            .find_layer_by_category_and_name(&category, &name, StateLayer::Published)
            .ok_or_else(|| {
                StatusError::not_found()
                    .brief("Layer not found")
                    .cause("The specified layer does not exist or is not published")
            })?
            .clone();

        let geometry = match lyr.geometry.as_str() {
            "points" => "circle".to_string(),
            "lines" => "line".to_string(),
            "polygons" => "fill".to_string(),
            _ => lyr.geometry.clone(),
        };

        (lyr, geometry)
    };

    let extent = query_extent(&lyr).await.unwrap_or_else(|e| {
        tracing::error!("Error querying extent: {:?}", e);
        Extent {
            xmin: -180.0,
            ymin: -90.0,
            xmax: 180.0,
            ymax: 90.0,
        }
    });

    let template = if is_minimal {
        MapLayerTemplateKind::Minimal(MapLayerMinimalTemplate {
            geometry: &geometry,
            layer: lyr,
            extent,
            base,
        })
    } else {
        MapLayerTemplateKind::Full(MapLayerTemplate {
            geometry: &geometry,
            layer: lyr,
            extent,
            base,
        })
    };

    res.render(Text::Html(
        template
            .render()
            .map_err(|e| StatusError::internal_server_error().cause(e.to_string()))?,
    ));
    Ok(())
}

#[handler]
pub async fn page_map_view(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> Result<(), StatusError> {
    let style_id = req
        .param::<String>("style_id")
        .ok_or_else(|| StatusError::bad_request().brief("Missing style_id parameter"))?;
    let is_minimal = req.query::<bool>("minimal").unwrap_or_default();

    let style = Style::from_id(&style_id)
        .await
        .map_err(|e| StatusError::internal_server_error().cause(e.to_string()))?;
    let (base, _) = make_base(depot).await;

    let base_url = base_url_from_request(req);
    let resolved_style_json = resolve_style_json(&style.style, &base_url)
        .map_err(|e| StatusError::internal_server_error().cause(e.to_string()))?
        .to_string();

    let template = if is_minimal {
        MapTemplate::Minimal(MapViewMinimalTemplate {
            base,
            style,
            resolved_style_json,
        })
    } else {
        MapTemplate::Full(MapViewTemplate {
            base,
            style,
            resolved_style_json,
        })
    };

    res.render(Text::Html(
        template
            .render()
            .map_err(|e| StatusError::internal_server_error().cause(e.to_string()))?,
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;
    use crate::models::category::Category;

    fn test_base() -> BaseTemplateData {
        let i18n = I18n::new();
        BaseTemplateData {
            is_auth: true,
            is_admin: true,
            translate: i18n.get_all_translations("en-US"),
            version: "0.0.0-test",
        }
    }

    fn test_style() -> Style {
        Style {
            id: "style-1".to_string(),
            name: "arrendamiento_rural_2024_token_mvt".to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "arrendamientos".to_string(),
                description: "".to_string(),
            },
            description: "".to_string(),
            style: r#"{
                "version": 8,
                "glyphs": "mvt://services/map_assets/glyphs/{fontstack}/{range}.pbf",
                "sources": {
                    "src_arrendamientos": {
                        "type": "vector",
                        "tiles": ["mvt://services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf"]
                    }
                }
            }"#
            .to_string(),
        }
    }

    /// Regression test: the map preview used to embed `style.style` (the raw
    /// stored document) directly, so `mvt://` tokens never got resolved and
    /// MapLibre tried to fetch tiles over a scheme the browser can't request
    /// — see the CORS errors the user hit loading a real style preview.
    #[test]
    fn mapview_template_embeds_resolved_urls_not_raw_mvt_tokens() {
        let style = test_style();
        let resolved_style_json = resolve_style_json(&style.style, "http://127.0.0.1:5887")
            .expect("style JSON must parse")
            .to_string();

        let template = MapViewTemplate {
            base: test_base(),
            style,
            resolved_style_json,
        };
        let html = template.render().expect("mapview.html must render");

        assert!(
            !html.contains("mvt://"),
            "resolved style must not leak raw mvt:// tokens into the preview page: {html}"
        );
        assert!(
            html.contains("http://127.0.0.1:5887/services/tiles/category/arrendamientos/{z}/{x}/{y}.pbf"),
            "expected the resolved tile URL in {html}"
        );
        assert!(
            html.contains("http://127.0.0.1:5887/services/map_assets/glyphs/{fontstack}/{range}.pbf"),
            "expected the resolved glyphs URL in {html}"
        );
    }
}
