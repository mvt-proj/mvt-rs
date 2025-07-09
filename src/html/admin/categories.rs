use std::collections::HashMap;

use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::User,
    error::{AppError, AppResult},
    get_categories,
    html::main::{BaseTemplateData, get_session_data},
    models::category::Category,
};

#[derive(Template)]
#[template(path = "admin/categories/categories.html")]
struct ListCategoriesTemplate<'a> {
    categories: &'a Vec<Category>,
    current_user: &'a User,
    base: BaseTemplateData,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewCategory<'a> {
    id: Option<String>,
    name: &'a str,
    description: String,
}

#[handler]
pub async fn list_categories(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (is_auth, user) = get_session_data(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };
    if user.is_none() {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    }
    let current_user = user.unwrap();

    let categories = get_categories().await.read().await;

    let template = ListCategoriesTemplate {
        categories: &categories,
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_category<'a>(
    res: &mut Response,
    new_category: NewCategory<'a>,
) -> AppResult<()> {
    let category = Category::new(
        new_category.name.to_string(),
        new_category.description.to_string(),
    )
    .await;

    if let Err(err) = category {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    category?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/categories"));
    Ok(())
}

#[handler]
pub async fn edit_category<'a>(res: &mut Response, new_category: NewCategory<'a>) -> AppResult<()> {
    let category = Category::from_id(new_category.id.as_ref().unwrap()).await;

    if let Err(err) = category {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let updated_category = category?
        .update_category(
            new_category.name.to_string(),
            new_category.description.to_string(),
        )
        .await;

    if let Err(err) = updated_category {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    updated_category?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/categories"));
    Ok(())
}

#[handler]
pub async fn delete_category(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("schema".to_string()));

    if let Err(err) = id {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    let id = id?;

    let category = Category::from_id(&id).await?;

    let deleted_category = category.delete_category().await;
    if let Err(err) = deleted_category {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    deleted_category?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/categories"));
    Ok(())
}
