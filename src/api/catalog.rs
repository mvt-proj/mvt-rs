use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth, get_catalog,
    models::{catalog::Layer, category::Category},
};

#[handler]
pub async fn list(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let catalog = get_catalog().await.read().await;
    let mut layers = catalog.layers.clone();
    let scheme = req.scheme().to_string();

    let host = req
        .headers()
        .get("host")
        .ok_or(AppError::RequestParamError("Missing host header".to_string()))?
        .to_str()
        .map_err(|_| AppError::RequestParamError("Invalid host header encoding".to_string()))?;

    for layer in &mut layers {
        layer.url = Some(format!(
            "{scheme}://{host}/services/tiles/{}:{}/{{z}}/{{x}}]/{{y}}].pbf",
            layer.category.name, layer.name
        ));
    }

    res.render(Json(&layers));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewLayerRequest {
    category: String,
    database_id: String,
    geometry: String,
    name: String,
    alias: String,
    description: String,
    schema: String,
    table: String,
    fields: Vec<String>,
    filter: Option<String>,
    srid: Option<u32>,
    geom: Option<String>,
    sql_mode: Option<String>,
    buffer: Option<u32>,
    extent: Option<u32>,
    zmin: Option<u32>,
    zmax: Option<u32>,
    zmax_do_not_simplify: Option<u32>,
    buffer_do_not_simplify: Option<u32>,
    extent_do_not_simplify: Option<u32>,
    clip_geom: Option<bool>,
    delete_cache_on_start: Option<bool>,
    max_cache_age: Option<u64>,
    max_records: Option<u64>,
    published: bool,
    groups: Option<Vec<String>>,
}

#[handler]
pub async fn create_layer(res: &mut Response, layer_form: NewLayerRequest) -> AppResult<()> {
    let category = Category::from_id(&layer_form.category).await?;

    let auth = get_auth().await.read().await;
    let groups: Vec<Group> = layer_form
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();
    drop(auth);

    let name = crate::services::utils::normalize_name(&layer_form.name)?;

    let layer = Layer {
        id: uuid::Uuid::new_v4().simple().to_string(),
        category,
        database_id: layer_form.database_id,
        geometry: layer_form.geometry,
        name,
        alias: layer_form.alias,
        description: layer_form.description,
        schema: layer_form.schema,
        table_name: layer_form.table,
        fields: layer_form.fields,
        filter: layer_form.filter,
        srid: layer_form.srid,
        geom: layer_form.geom,
        sql_mode: layer_form.sql_mode,
        buffer: layer_form.buffer,
        extent: layer_form.extent,
        zmin: layer_form.zmin,
        zmax: layer_form.zmax,
        zmax_do_not_simplify: layer_form.zmax_do_not_simplify,
        buffer_do_not_simplify: layer_form.buffer_do_not_simplify,
        extent_do_not_simplify: layer_form.extent_do_not_simplify,
        clip_geom: layer_form.clip_geom,
        delete_cache_on_start: layer_form.delete_cache_on_start,
        max_cache_age: layer_form.max_cache_age,
        max_records: layer_form.max_records,
        published: layer_form.published,
        url: None,
        groups: Some(groups),
    };

    let mut catalog = get_catalog().await.write().await;
    catalog.add_layer(layer.clone()).await?;
    res.render(Json(&layer));
    Ok(())
}
