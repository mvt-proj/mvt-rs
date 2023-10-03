use serde::{Deserialize, Serialize};
use askama::Template;
use salvo::prelude::*;

use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    catalog::{Catalog, Layer, StateLayer},
    get_app_state,
    get_catalog,
};

#[derive(Template)]
#[template(path = "admin/catalog.html")]
struct CatalogTemplate<'a> {
    layers: &'a Vec<Layer>,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewLayer<'a> {
    geometry: &'a str,
    name: String,
    alias: String,
    schema: Option<String>,
    table: String,
    fields: Vec<String>,
    filter: Option<String>,
    srid: Option<u32>,
    geom: Option<String>,
    buffer: Option<u32>,
    extent: Option<u32>,
    zmin: Option<u32>,
    zmax: Option<u32>,
    zmax_do_not_simplify: Option<u32>,
    buffer_do_not_simplify: Option<u32>,
    extent_do_not_simplify: Option<u32>,
    clip_geom: Option<bool>,
    delete_cache_on_start: Option<bool>,
    /// max_cache_age: on seconds: default 0 -> infinite
    max_cache_age: Option<u64>,
    published: bool,

}



#[handler]
pub async fn page_catalog(res: &mut Response) {
    let catalog: Catalog = get_catalog().clone();

    let template = CatalogTemplate {
        layers: &catalog.layers,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn create_layer<'a>(res: &mut Response, new_layer: NewLayer<'a>) {

    let app_state = get_app_state();

    let layer = Layer {
        geometry: new_layer.geometry.to_string(),
        name: new_layer.name,
        alias: new_layer.alias,
        schema: new_layer.schema,
        table: new_layer.table,
        fields: new_layer.fields,
        filter: new_layer.filter,
        srid: new_layer.srid,
        geom: new_layer.geom,
        buffer: new_layer.buffer,
        extent: new_layer.extent,
        zmin: new_layer.zmin,
        zmax: new_layer.zmax,
        zmax_do_not_simplify: new_layer.zmax_do_not_simplify,
        buffer_do_not_simplify: new_layer.buffer_do_not_simplify,
        extent_do_not_simplify: new_layer.extent_do_not_simplify,
        clip_geom: new_layer.clip_geom,
        delete_cache_on_start: new_layer.delete_cache_on_start,
        max_cache_age: new_layer.max_cache_age,
        published: new_layer.published,
    };

    app_state.catalog.layers.push(layer);

    let json_str = serde_json::to_string(&app_state.catalog.layers).unwrap();
    let file_path = Path::new(&app_state.catalog.config_dir).join("catalog.json");
    let mut file = File::create(file_path).await.unwrap();
    file.write_all(json_str.as_bytes()).await.unwrap();
    file.flush().await.unwrap();
    res.render(Redirect::other("/admin/catalog"));
}

#[handler]
pub async fn swich_published(req: &mut Request, res: &mut Response) {
    let app_state = get_app_state();

    let layer_name = req.param::<String>("layer_name").unwrap();
    let layer = app_state.catalog
        .find_layer_by_name(&layer_name, StateLayer::ANY)
        .unwrap();

    let mut cloned_layer = layer.clone();
    cloned_layer.published = !cloned_layer.published;

    app_state.catalog.remove_layer_by_name(&layer_name);
    app_state.catalog.layers.push(cloned_layer);

    let json_str = serde_json::to_string(&app_state.catalog.layers).unwrap();
    let file_path = Path::new(&app_state.catalog.config_dir).join("catalog.json");
    let mut file = File::create(file_path).await.unwrap();
    file.write_all(json_str.as_bytes()).await.unwrap();
    file.flush().await.unwrap();
    res.render(Redirect::other("/admin/catalog"));
}
