use crate::{
    auth::{Group, User},
    catalog::{Layer, StateLayer},
    error::{AppError, AppResult},
    get_auth, get_catalog,
};
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {}

#[derive(Template)]
#[template(path = "admin/users/new.html")]
struct NewUserTemplate {
    groups: Vec<Group>,
}

#[derive(Template)]
#[template(path = "admin/users/edit.html")]
struct EditUserTemplate {
    user: User,
    groups: Vec<Group>,
}

#[derive(Template)]
#[template(path = "admin/catalog/layers/new.html")]
struct NewLayerTemplate {}

#[derive(Template)]
#[template(path = "admin/catalog/layers/edit.html")]
struct EditLayerTemplate {
    layer: Layer,
}

#[handler]
pub async fn index(res: &mut Response) -> AppResult<()> {
    let template = IndexTemplate {};
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_user(res: &mut Response) -> AppResult<()> {
    let auth = get_auth().clone();

    let template = NewUserTemplate {
        groups: auth.groups,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_user(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("username".to_string()))?;
    let auth = get_auth().clone();
    let user = auth
        .get_user_by_id(&id)
        .ok_or_else(|| AppError::UserNotFoundError(id.clone()))?;

    let template = EditUserTemplate {
        user: user.clone(),
        groups: auth.groups,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_layer(res: &mut Response) -> AppResult<()> {
    let template = NewLayerTemplate {};
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_layer(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;
    let catalog = get_catalog().clone();
    let layer = catalog
        .find_layer_by_id(&layer_id, StateLayer::Any)
        .unwrap();
    let template = EditLayerTemplate {
        layer: layer.clone(),
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
