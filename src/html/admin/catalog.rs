use askama::Template;
use salvo::prelude::*;

use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    catalog::{Catalog, Layer},
    get_app_state, get_catalog,
};

#[derive(Template)]
#[template(path = "admin/catalog.html")]
struct CatalogTemplate<'a> {
    layers: &'a Vec<Layer>,
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
pub async fn create_layer() -> Result<String, anyhow::Error> {
    let app_state = get_app_state();
    let layer = Layer {
        geometry: "polygons".to_string(),
        name: "cp_manzanas".to_string(),
        alias: "cp_manzanas".to_string(),
        schema: Some("public".to_string()),
        table: "carlos_paz_manzanas".to_string(),
        fields: vec![
            "id".to_string(),
            "circunscri".to_string(),
            "seccion".to_string(),
            "manzana".to_string(),
        ],
        filter: None,
        srid: Some(4326),
        zmin: Some(10),
        zmax: Some(18),
        geom: Some("geom".to_string()),
        buffer: Some(256),
        extent: Some(4096),
        zmax_do_not_simplify: None,
        buffer_do_not_simplify: None,
        extent_do_not_simplify: None,
        clip_geom: Some(true),
        delete_cache_on_start: Some(true),
        max_cache_age: Some(0),
        published: true,
    };

    app_state.catalog.layers.push(layer);

    let json_str = serde_json::to_string(&app_state.catalog.layers)?;
    let file_path = Path::new(&app_state.catalog.config_dir).join("catalog.json");
    let mut file = File::create(file_path).await?;
    file.write_all(json_str.as_bytes()).await?;
    file.flush().await?;
    Ok(json_str)
}
