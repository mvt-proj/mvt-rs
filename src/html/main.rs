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
pub async fn page_catalog(res: &mut Response) {
    let catalog: Catalog = get_catalog().clone();

    let template = CatalogTemplate {
        layers: &catalog.published_layers,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn page_map(req: &mut Request, res: &mut Response) {
    let catalog: Catalog = get_catalog().clone();

    let layer_name = req.param::<String>("layer_name").unwrap();
    let layer = catalog
        .find_layer_by_name(&layer_name, StateLayer::PUBLISHED)
        .unwrap();
    let geometry = match layer.geometry.as_str() {
        "points" => "circle",
        "lines" => "line",
        "polygons" => "fill",
        _ => &layer.geometry,
    };

    let template = MapTemplate {
        name: &layer.name,
        alias: &layer.alias,
        geometry,
    };
    res.render(Text::Html(template.render().unwrap()));
}
