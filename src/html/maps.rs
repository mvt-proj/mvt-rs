use super::utils::{BaseTemplateData, is_authenticated};
use crate::db::metadata::{Extent, query_extent};
use crate::get_catalog;
use crate::models::catalog::{Layer, StateLayer};
use crate::models::styles::Style;
use askama::Template;
use salvo::prelude::*;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "maplayer.html")]
struct MapLayerTemplate<'a> {
    geometry: &'a str,
    layer: Layer,
    extent: Extent,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "mapview.html")]
struct MapViewTemplate {
    base: BaseTemplateData,
    style: Style,
}

#[derive(Template)]
#[template(path = "mapview_minimal.html")]
struct MapViewMinimalTemplate {
    base: BaseTemplateData,
    style: Style,
}

enum MapTemplate {
    Minimal(MapViewMinimalTemplate),
    Full(MapViewTemplate),
}

impl MapTemplate {
    fn render(&self) -> String {
        match self {
            MapTemplate::Minimal(tpl) => tpl.render().unwrap(),
            MapTemplate::Full(tpl) => tpl.render().unwrap(),
        }
    }
}

#[handler]
pub async fn page_map_layer(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> Result<(), StatusError> {
    let layer_name = req.param::<String>("layer_name").unwrap();
    let parts: Vec<&str> = layer_name.split(':').collect();
    let category = parts.first().unwrap_or(&"").to_string();
    let name = parts.get(1).unwrap_or(&"").to_string();

    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

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

    let template = MapLayerTemplate {
        geometry: &geometry,
        layer: lyr,
        extent,
        base,
    };

    res.render(Text::Html(template.render().unwrap()));
    Ok(())
}

#[handler]
pub async fn page_map_view(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> Result<(), StatusError> {
    let style_id = req.param::<String>("style_id").unwrap();
    let is_minimal = req.query::<bool>("minimal").unwrap_or_default();

    let style = Style::from_id(&style_id).await.unwrap();
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let template = if is_minimal {
        MapTemplate::Minimal(MapViewMinimalTemplate { base, style })
    } else {
        MapTemplate::Full(MapViewTemplate { base, style })
    };

    res.render(Text::Html(template.render()));
    Ok(())
}
