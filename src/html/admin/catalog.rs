use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{Auth, Group, User},
    error::{AppError, AppResult},
    get_app_state, get_auth, get_catalog,
    html::main::BaseTemplateData,
    models::{
        catalog::{Catalog, Layer},
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
#[template(path = "admin/catalog/table.html")]
struct CatalogTableTemplate<'a> {
    layers: &'a Vec<Layer>,
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
    let mut is_auth = false;
    let mut user: Option<User> = None;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(usr) = auth.get_user_by_id(&userid) {
                is_auth = true;
                user = Some(usr.clone());
            }
        }
    }

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
pub async fn table_catalog(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");

    let mut catalog: Catalog = get_catalog().clone();
    let mut is_auth = false;
    let mut user: Option<User> = None;

    if let Some(filter) = filter {
        catalog.layers = catalog
            .layers
            .iter()
            .filter(|layer| {
                layer.alias.to_lowercase().contains(&filter.to_lowercase())
                    || layer
                        .category
                        .name
                        .to_lowercase()
                        .contains(&filter.to_lowercase())
            })
            .cloned()
            .collect();
    }

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(usr) = auth.get_user_by_id(&userid) {
                is_auth = true;
                user = Some(usr.clone());
            }
        }
    }

    let base = BaseTemplateData { is_auth };
    let current_user = user.unwrap();

    let template = CatalogTableTemplate {
        layers: &catalog.layers,
        current_user: &current_user,
        base,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn create_layer<'a>(res: &mut Response, new_layer: NewLayer<'a>) -> AppResult<()> {
    let app_state = get_app_state();
    let uuid = Uuid::new_v4();
    let hex_string = uuid.simple().to_string();

    let category = Category::from_id(&new_layer.category).await?;

    // let selected_groups: Vec<Group> = new_layer
    //     .groups
    //     .iter()
    //     .filter_map(|group_name| app_state.auth.find_group_by_name(group_name).cloned())
    //     .collect();

    let selected_groups: Vec<Group> = new_layer
        .groups
        .as_ref()
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group_name| app_state.auth.find_group_by_name(group_name).cloned())
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

    app_state.catalog.add_layer(layer).await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn update_layer<'a>(res: &mut Response, new_layer: NewLayer<'a>) -> AppResult<()> {
    let app_state = get_app_state();

    let category = Category::from_id(&new_layer.category).await?;

    // let selected_groups: Vec<Group> = new_layer
    //     .groups
    //     .iter()
    //     .filter_map(|group_name| app_state.auth.find_group_by_name(group_name).cloned())
    //     .collect();
    //
    let selected_groups: Vec<Group> = new_layer
        .groups
        .as_ref()
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group_name| app_state.auth.find_group_by_name(group_name).cloned())
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

    app_state.catalog.update_layer(layer).await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn delete_layer<'a>(res: &mut Response, req: &mut Request) -> AppResult<()> {
    let app_state = get_app_state();

    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    app_state.catalog.delete_layer(layer_id).await?;
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}

#[handler]
pub async fn swich_published(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let app_state = get_app_state();

    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    app_state.catalog.swich_layer_published(&layer_id).await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/catalog"));
    Ok(())
}
