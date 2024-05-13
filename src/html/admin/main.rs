use crate::{
    error::{AppResult, AppError},
    auth::User,
    catalog::{Layer, StateLayer},
    get_auth, get_catalog,
};
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {}

#[derive(Template)]
#[template(path = "admin/users/new.html")]
struct NewUserTemplate {}

#[derive(Template)]
#[template(path = "admin/users/edit.html")]
struct EditUserTemplate {
    user: User,
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
    let template = NewUserTemplate {};
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_user(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let username = req.param::<String>("username").ok_or(AppError::RequestParamError("username".to_string()))?;
    let auth = get_auth().clone();
    let user = auth.find_user_by_name(&username)
        .ok_or_else(|| AppError::UserNotFoundError(username.clone()))?;

    let template = EditUserTemplate { user: user.clone() };
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
    let layer_name = req.param::<String>("layer_name").ok_or(AppError::RequestParamError("layer_name".to_string()))?;
    let catalog = get_catalog().clone();
    let layer = catalog
        .find_layer_by_name(&layer_name, StateLayer::Any)
        .unwrap();
    let template = EditLayerTemplate {
        layer: layer.clone(),
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
