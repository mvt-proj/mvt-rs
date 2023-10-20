use askama::Template;
use salvo::prelude::*;

use crate::{
    catalog::{Catalog, Layer, StateLayer},
    get_catalog,
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {}

#[derive(Template)]
#[template(path = "error404.html")]
struct E404Template {}

#[derive(Template)]
#[template(path = "catalog.html")]
struct CatalogTemplate<'a> {
    layers: &'a Vec<Layer>,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    name: &'a str,
    alias: &'a str,
    geometry: &'a str,
}

#[handler]
pub async fn index(res: &mut Response) {
    let template = IndexTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn error404(res: &mut Response) {
    let template = E404Template {};
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn page_catalog(res: &mut Response) {
    let catalog: Catalog = get_catalog().clone();

    let template = CatalogTemplate {
        layers: &catalog.get_published_layers(),
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn page_map(req: &mut Request, res: &mut Response) -> Result<(), StatusError> {
    let catalog: Catalog = get_catalog().clone();
    let layer_name = req.param::<String>("layer_name").unwrap();

    let lyr = catalog
        .find_layer_by_name(&layer_name, StateLayer::PUBLISHED)
        .ok_or_else(|| {
            StatusError::not_found()
                .brief("Layer not found")
                .cause("The specified layer does not exist or is not published")
        })?;

    let geometry = match lyr.geometry.as_str() {
        "points" => "circle",
        "lines" => "line",
        "polygons" => "fill",
        _ => &lyr.geometry,
    };

    let template = MapTemplate {
        name: &lyr.name,
        alias: &lyr.alias,
        geometry,
    };

    res.render(Text::Html(template.render().unwrap()));
    Ok(())
}
