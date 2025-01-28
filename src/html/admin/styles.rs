use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::User,
    error::{AppError, AppResult},
    html::main::{get_session_data, BaseTemplateData},
    models::{category::Category, styles::Style},
};

#[derive(Template)]
#[template(path = "admin/styles/styles.html")]
struct ListStylesTemplate<'a> {
    current_user: &'a User,
    base: BaseTemplateData,
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
pub async fn list_styles(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (is_auth, user) = get_session_data(depot);

    let base = BaseTemplateData { is_auth };
    let current_user = user.unwrap();

    let template = ListStylesTemplate {
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_style<'a>(res: &mut Response, new_style: NewStyle<'a>) -> AppResult<()> {
    let category = Category::from_id(&new_style.category).await;

    if let Err(err) = category {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    let result = Style::new(
        new_style.name.to_string(),
        category.unwrap(),
        new_style.description.to_string(),
        new_style.style.to_string(),
    )
    .await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn edit_style<'a>(res: &mut Response, new_style: NewStyle<'a>) -> AppResult<()> {
    let style = Style::from_id(&new_style.id.unwrap()).await;

    if let Err(err) = style {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let category = Category::from_id(&new_style.category).await;

    if let Err(err) = category {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    let result = style
        .unwrap()
        .update_style(
            new_style.name.to_string(),
            category.unwrap(),
            new_style.description.to_string(),
            new_style.style.to_string(),
        )
        .await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn delete_style(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let style = Style::from_id(&id).await;

    if let Err(err) = style {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let result = style.unwrap().delete_style().await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}
