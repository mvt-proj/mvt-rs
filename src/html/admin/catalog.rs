use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{Group, User},
    error::{AppError, AppResult},
    get_auth, get_cache_wrapper, get_catalog, get_categories, get_db_registry,
    html::utils::{BaseTemplateData, make_base},
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
    },
};

#[derive(Template)]
#[template(path = "admin/catalog/catalog.html")]
struct CatalogTemplate<'a> {
    current_user: &'a User,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/catalog/layers/new.html")]
struct NewLayerTemplate {
    categories: Vec<Category>,
    groups: Vec<Group>,
    databases: Vec<(String, String)>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/catalog/layers/edit.html")]
struct EditLayerTemplate {
    layer: Layer,
    categories: Vec<Category>,
    groups: Vec<Group>,
    databases: Vec<(String, String)>,
    base: BaseTemplateData,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewLayer<'a> {
    id: String,
    category: String,
    database_id: String,
    geometry: &'a str,
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
    /// max_cache_age: on seconds: default 0 -> infinite
    max_cache_age: Option<u64>,
    max_records: Option<u64>,
    published: bool,
    groups: Option<Vec<String>>,
}

#[handler]
pub async fn catalog_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;
    let Some(current_user) = user else {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };
    let template = CatalogTemplate {
        current_user: &current_user,
        base,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn new_layer_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let categories = get_categories().await.read().await;
    let auth = get_auth().await.read().await;
    let groups = auth.groups.clone();
    let databases = get_db_registry().list_databases();
    let (base, _) = make_base(depot).await;

    let template = NewLayerTemplate {
        categories: (categories).to_vec(),
        groups,
        databases,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_layer_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let categories = get_categories().await.read().await;
    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;
    let catalog = get_catalog().await.read().await;
    let auth = get_auth().await.read().await;
    let groups = auth.groups.clone();
    let layer = catalog
        .find_layer_by_id(&layer_id, StateLayer::Any)
        .ok_or(AppError::NotFound(format!("Layer {layer_id} not found")))?;
    let databases = get_db_registry().list_databases();
    let template = EditLayerTemplate {
        layer: layer.clone(),
        categories: (categories).to_vec(),
        groups,
        databases,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_layer<'a>(res: &mut Response, layer_form: NewLayer<'a>) -> AppResult<()> {
    let uuid = Uuid::new_v4();
    let hex_string = uuid.simple().to_string();

    let category = Category::from_id(&layer_form.category).await;

    if let Err(err) = category {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let category = category?;
    let auth = get_auth().await.read().await;

    let selected_groups: Vec<Group> = layer_form
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
        database_id: layer_form.database_id,
        geometry: layer_form.geometry.to_string(),
        name: layer_form.name,
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
pub async fn update_layer<'a>(res: &mut Response, layer_form: NewLayer<'a>) -> AppResult<()> {
    let category = Category::from_id(&layer_form.category).await;

    if let Err(err) = category {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let category = category?;
    let auth = get_auth().await.read().await;

    let selected_groups: Vec<Group> = layer_form
        .groups
        .as_ref()
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
                .collect::<Vec<Group>>()
        })
        .unwrap_or_default();

    let layer_key = format!("{}_{}", category.name, layer_form.name);

    let layer = Layer {
        id: layer_form.id,
        category,
        database_id: layer_form.database_id,
        geometry: layer_form.geometry.to_string(),
        name: layer_form.name,
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
        groups: Some(selected_groups),
    };

    let mut catalog = get_catalog().await.write().await;
    let layer = catalog.update_layer(layer).await;

    if let Err(err) = layer {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    // The layer config changed (fields, filter, sql_mode, ...): invalidate its
    // tile cache exactly like the manual "clear cache" action. This bumps the
    // layer version so the ETag changes (forcing browsers/QGIS to refetch) and
    // removes the stale tiles, so the next request regenerates them from the DB
    // with the updated columns.
    get_cache_wrapper().delete_layer_cache(&layer_key).await?;

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
            // layer.name.clone()
            format!("{}_{}", layer.category.name, layer.name)
        } else {
            res.status_code(StatusCode::BAD_REQUEST);
            return Err(AppError::CacheNotFound(layer_id.to_string()));
        }
    };

    let cache_wrapper = get_cache_wrapper();
    cache_wrapper.delete_layer_cache(&layer_name).await?;

    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}
