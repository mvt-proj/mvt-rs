use askama::Template;
use salvo::prelude::*;

use crate::{Config, LayersConfig};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    layers_config: &'a LayersConfig,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    name: &'a str,
    alias: &'a str,
    geometry: &'a str,
}

#[handler]
pub async fn index(depot: &mut Depot, res: &mut Response) {
    let config = depot.obtain::<Config>().unwrap();
    let config = config.clone();
    let layers_config: LayersConfig = config.layers_config;

    let template = IndexTemplate {
        layers_config: &layers_config,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn mapview(req: &mut Request, res: &mut Response, depot: &mut Depot) {
    let config = depot.obtain::<Config>().unwrap();
    let config = config.clone();
    let layers_config: LayersConfig = config.layers_config;

    let layer_name = req.param::<String>("layer").unwrap();
    let layer = layers_config.find_layer_by_name(&layer_name).unwrap();
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
