use crate::{
    auth::{Group, User},
    error::{AppError, AppResult},
    get_auth, get_catalog, get_categories,
    html::main::{is_authenticated, BaseTemplateData},
    models::{
        catalog::{Layer, StateLayer},
        category::Category,
        styles::Style,
    },
};
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/users/new.html")]
struct NewUserTemplate {
    groups: Vec<Group>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/users/edit.html")]
struct EditUserTemplate {
    user: User,
    groups: Vec<Group>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/catalog/layers/new.html")]
struct NewLayerTemplate {
    categories: Vec<Category>,
    groups: Vec<Group>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/catalog/layers/edit.html")]
struct EditLayerTemplate {
    layer: Layer,
    categories: Vec<Category>,
    groups: Vec<Group>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/categories/new.html")]
struct NewCategoryTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/categories/edit.html")]
struct EditCategoryTemplate {
    category: Category,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/styles/new.html")]
struct NewStyleTemplate {
    categories: Vec<Category>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/styles/edit.html")]
struct EditStyleTemplate {
    style: Style,
    categories: Vec<Category>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/groups/new.html")]
struct NewGroupTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/groups/edit.html")]
struct EditGroupTemplate {
    group: Group,
    base: BaseTemplateData,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };
    let template = IndexTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_user(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;

    let base = BaseTemplateData { is_auth };
    let auth = get_auth().await.read().await;

    let template = NewUserTemplate {
        groups: auth.groups.clone(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_user(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("username".to_string()))?;
    let auth = get_auth().await.read().await;
    let user = auth
        .get_user_by_id(&id)
        .ok_or_else(|| AppError::UserNotFoundError(id.clone()))?;

    let template = EditUserTemplate {
        user: user.clone(),
        groups: auth.groups.clone(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_layer(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let categories = get_categories().await.read().await;
    let auth = get_auth().await.read().await;
    let groups = auth.groups.clone();
    let is_auth = is_authenticated(depot).await;

    let base = BaseTemplateData { is_auth };

    let template = NewLayerTemplate {
        categories: (&categories).to_vec(),
        groups,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_layer(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let categories = get_categories().await.read().await;
    let layer_id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;
    let catalog = get_catalog().await.read().await;
    let auth = get_auth().await.read().await;
    let groups = auth.groups.clone();
    let layer = catalog
        .find_layer_by_id(&layer_id, StateLayer::Any)
        .unwrap();
    let template = EditLayerTemplate {
        layer: layer.clone(),
        categories: (&categories).to_vec(),
        groups,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_category(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let template = NewCategoryTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_category(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let category = Category::from_id(&id).await?;
    let template = EditCategoryTemplate {
        category: category.clone(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_style(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;

    let base = BaseTemplateData { is_auth };

    let categories = get_categories().await.read().await;
    let template = NewStyleTemplate {
        categories: (&categories).to_vec(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_style(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    let categories = get_categories().await.read().await;
    let template = EditStyleTemplate {
        style: style.clone(),
        categories: (&categories).to_vec(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_group(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let template = NewGroupTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_group(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let base = BaseTemplateData { is_auth };

    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let group = Group::from_id(&id).await?;
    let template = EditGroupTemplate {
        group: group.clone(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
