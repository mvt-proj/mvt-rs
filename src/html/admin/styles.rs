use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, User},
    category::Category,
    error::{AppError, AppResult}, styles::Style,
};

#[derive(Template)]
#[template(path = "admin/styles/styles.html")]
struct ListCategoriesTemplate<'a> {
    styles: &'a Vec<Style>,
    current_user: &'a User,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewStyle<'a> {
    id: Option<String>,
    name: &'a str,
    category: String,
    description: &'a str,
    style: String,
}

#[handler]
pub async fn list_styles(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let authorization = req.headers().get("authorization").unwrap();
    let authorization_str = authorization
        .to_str()
        .map_err(|err| AppError::ConversionError(err.to_string()))?;

    let auth: Auth = crate::get_auth().clone();
    let current_user = auth.get_current_user(authorization_str).unwrap();
    let styles = Style::get_all_styles().await?;
    let template = ListCategoriesTemplate {
        styles: &styles,
        current_user,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_style<'a>(
    res: &mut Response,
    new_style: NewStyle<'a>,
) -> AppResult<()> {
    Style::new(
        new_style.name.to_string(),
        Category::from_id(&new_style.category).await?,
        new_style.description.to_string(),
        new_style.style.to_string(),
    ).await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn edit_style<'a>(res: &mut Response, new_style: NewStyle<'a>) -> AppResult<()> {
    let style = Style::from_id(&new_style.id.unwrap()).await?;
    style.update_style(
        new_style.name.to_string(),
        Category::from_id(&new_style.category).await?,
        new_style.description.to_string(),
        new_style.style.to_string(),
    ).await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn delete_style(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    
    let style = Style::from_id(&id).await?;
    style.delete_style().await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}