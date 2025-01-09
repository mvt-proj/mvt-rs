use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, User},
    error::{AppError, AppResult},
    models::category::Category,
};

#[derive(Template)]
#[template(path = "admin/categories/categories.html")]
struct ListCategoriesTemplate<'a> {
    categories: &'a Vec<Category>,
    current_user: &'a User,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewCategory<'a> {
    id: Option<String>,
    name: &'a str,
    description: String,
}

#[handler]
pub async fn list_categories(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let app_state = crate::get_app_state();
    let authorization = req.headers().get("authorization").unwrap();
    let authorization_str = authorization
        .to_str()
        .map_err(|err| AppError::ConversionError(err.to_string()))?;

    let auth: Auth = crate::get_auth().clone();
    let current_user = auth.get_current_user(authorization_str).unwrap();
    let template = ListCategoriesTemplate {
        categories: &app_state.categories,
        current_user,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_category<'a>(
    res: &mut Response,
    new_category: NewCategory<'a>,
) -> AppResult<()> {
    Category::new(
        new_category.name.to_string(),
        new_category.description.to_string(),
    )
    .await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/categories"));
    Ok(())
}

#[handler]
pub async fn edit_category<'a>(res: &mut Response, new_category: NewCategory<'a>) -> AppResult<()> {
    let category = Category::from_id(new_category.id.as_ref().unwrap()).await?;
    category
        .update_category(
            new_category.name.to_string(),
            new_category.description.to_string(),
        )
        .await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/categories"));
    Ok(())
}

#[handler]
pub async fn delete_category(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    let category = Category::from_id(&id).await?;

    category.delete_category().await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/categories"));
    Ok(())
}
