use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{Group, User}, error::{AppError, AppResult}, get_auth, get_cache_wrapper, get_catalog, html::main::{get_session_data, BaseTemplateData}, models::{
        catalog::{Layer, StateLayer},
        category::Category,
    }
};

#[derive(Template)]
#[template(path = "admin/catalog/catalog.html")]
struct CatalogTemplate<'a> {
    current_user: &'a User,
    base: BaseTemplateData,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewLayer<'a> {
    id: String,
    category: String,
    geometry: &'a str,
    name: String,
    alias: String,
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
    /// max_cache_age: on seconds: default 0 -> infinite
    max_cache_age: Option<u64>,
    published: bool,
    groups: Option<Vec<String>>,
}

#[handler]
pub async fn page_catalog(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (is_auth, user) = get_session_data(depot).await;
    let base = BaseTemplateData { is_auth };
    let current_user = user.unwrap();
    let template = CatalogTemplate {
        current_user: &current_user,
        base,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn create_layer<'a>(res: &mut Response, new_layer: NewLayer<'a>) -> AppResult<()> {
    let uuid = Uuid::new_v4();
    let hex_string = uuid.simple().to_string();

    let category = Category::from_id(&new_layer.category).await;

    if let Err(err) = category {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let category = category?;
    let auth = get_auth().await.read().await;

    let selected_groups: Vec<Group> = new_layer
        .groups
        .as_ref()
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
                .collect::<Vec<Group>>()
        })
        .unwrap_or_default();

    let layer = Layer {
        id: hex_string,
        category,
        geometry: new_layer.geometry.to_string(),
        name: new_layer.name,
        alias: new_layer.alias,
        schema: new_layer.schema,
        table_name: new_layer.table,
        fields: new_layer.fields,
        filter: new_layer.filter,
        srid: new_layer.srid,
        geom: new_layer.geom,
        sql_mode: new_layer.sql_mode,
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
        url: None,
        groups: Some(selected_groups),
    };

    let mut catalog = get_catalog().await.write().await;
    let layer = catalog.add_layer(layer).await;

    if let Err(err) = layer {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn update_layer<'a>(res: &mut Response, new_layer: NewLayer<'a>) -> AppResult<()> {
    let category = Category::from_id(&new_layer.category).await;

    if let Err(err) = category {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let category = category?;
    let auth = get_auth().await.read().await;

    let selected_groups: Vec<Group> = new_layer
        .groups
        .as_ref()
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
                .collect::<Vec<Group>>()
        })
        .unwrap_or_default();

    let layer = Layer {
        id: new_layer.id,
        category,
        geometry: new_layer.geometry.to_string(),
        name: new_layer.name,
        alias: new_layer.alias,
        schema: new_layer.schema,
        table_name: new_layer.table,
        fields: new_layer.fields,
        filter: new_layer.filter,
        srid: new_layer.srid,
        geom: new_layer.geom,
        sql_mode: new_layer.sql_mode,
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
        url: None,
        groups: Some(selected_groups),
    };

    let mut catalog = get_catalog().await.write().await;
    let layer = catalog.update_layer(layer).await;

    if let Err(err) = layer {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn delete_layer<'a>(res: &mut Response, req: &mut Request) -> AppResult<()> {

    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let mut catalog = get_catalog().await.write().await;
    let layer = catalog.delete_layer(layer_id).await;

    if let Err(err) = layer {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn swich_published(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let mut catalog = get_catalog().await.write().await; // 🔓 Bloque limitado

    let layer = catalog.swich_layer_published(&layer_id).await;

    if let Err(err) = layer {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn delete_layer_cache<'a>(res: &mut Response, req: &mut Request) -> AppResult<()> {

    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let layer_name = {
        let catalog = get_catalog().await.read().await;
        if let Some(layer) = catalog.find_layer_by_id(&layer_id, StateLayer::Any) {
            layer.name.clone()
        } else {
            res.status_code(StatusCode::BAD_REQUEST);
            return Err(AppError::CacheNotFount(format!("{layer_id}")));
        }
    };

    let cache_wrapper = get_cache_wrapper();
    cache_wrapper.delete_layer_cache(&layer_name).await?;

    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}
