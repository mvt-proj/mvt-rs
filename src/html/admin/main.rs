use crate::{
    auth::{Group, User},
    models::{category::Category, styles::Style, catalog::{Layer, StateLayer}},
    error::{AppError, AppResult},
    get_auth,
    get_catalog,
    get_categories,
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
struct NewLayerTemplate {
    categories: Vec<Category>,
    groups: Vec<Group>,
}

#[derive(Template)]
#[template(path = "admin/catalog/layers/edit.html")]
struct EditLayerTemplate {
    layer: Layer,
    categories: Vec<Category>,
    groups: Vec<Group>,
}

#[derive(Template)]
#[template(path = "admin/categories/new.html")]
struct NewCategoryTemplate {}

#[derive(Template)]
#[template(path = "admin/categories/edit.html")]
struct EditCategoryTemplate {
    category: Category,
}

#[derive(Template)]
#[template(path = "admin/styles/new.html")]
struct NewStyleTemplate {
    categories: Vec<Category>,
}

#[derive(Template)]
#[template(path = "admin/styles/edit.html")]
struct EditStyleTemplate {
    style: Style,
    categories: Vec<Category>,
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
    let categories = get_categories().clone();
    let groups = get_auth().groups.clone();
    let template = NewLayerTemplate {
        categories,
        groups,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_layer(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let categories = get_categories().clone();
    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;
    let catalog = get_catalog().clone();
    let groups = get_auth().groups.clone();
    let layer = catalog
        .find_layer_by_id(&layer_id, StateLayer::Any)
        .unwrap();
    let template = EditLayerTemplate {
        layer: layer.clone(),
        categories,
        groups,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_category(res: &mut Response) -> AppResult<()> {
    let template = NewCategoryTemplate {};
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_category(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let category = Category::from_id(&id).await?;
    let template = EditCategoryTemplate {
        category: category.clone(),
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_style(res: &mut Response) -> AppResult<()> {
    let categories = get_categories().clone();
    let template = NewStyleTemplate {
        categories,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_style(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    let categories = get_categories().clone();
    let template = EditStyleTemplate {
        style: style.clone(),
        categories,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
