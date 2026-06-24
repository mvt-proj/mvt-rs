use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::User,
    error::{AppError, AppResult},
    get_categories,
    html::utils::{BaseTemplateData, make_base},
    models::{category::Category, styles::Style},
};

#[derive(Template)]
#[template(path = "admin/styles/styles.html")]
struct ListStylesTemplate<'a> {
    current_user: &'a User,
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
    let (base, user) = make_base(depot).await;
    let Some(current_user) = user else {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };
    let template = ListStylesTemplate {
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_style_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let categories = get_categories().await.read().await;
    let template = NewStyleTemplate {
        categories: (categories).to_vec(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_style_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    let categories = get_categories().await.read().await;
    let template = EditStyleTemplate {
        style: style.clone(),
        categories: (categories).to_vec(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_style<'a>(res: &mut Response, style_form: NewStyle<'a>) -> AppResult<()> {
    let category = match Category::from_id(&style_form.category).await {
        Ok(cat) => cat,
        Err(err) => {
            res.status_code(StatusCode::BAD_REQUEST);
            return Err(err);
        }
    };

    let result = Style::new(
        style_form.name.to_string(),
        category,
        style_form.description.to_string(),
        style_form.style.to_string(),
    )
    .await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    crate::reload_styles_cache().await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn update_style<'a>(res: &mut Response, style_form: NewStyle<'a>) -> AppResult<()> {
    let style_id = style_form
        .id
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = match Style::from_id(&style_id).await {
        Ok(s) => s,
        Err(err) => {
            res.status_code(StatusCode::NOT_FOUND);
            return Err(err);
        }
    };

    let category = match Category::from_id(&style_form.category).await {
        Ok(cat) => cat,
        Err(err) => {
            res.status_code(StatusCode::BAD_REQUEST);
            return Err(err);
        }
    };

    let result = style
        .update_style(
            style_form.name.to_string(),
            category,
            style_form.description.to_string(),
            style_form.style.to_string(),
        )
        .await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    crate::reload_styles_cache().await?;
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

    let style = match Style::from_id(&id).await {
        Ok(s) => s,
        Err(err) => {
            res.status_code(StatusCode::NOT_FOUND);
            return Err(err);
        }
    };

    let result = style.delete_style().await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    crate::reload_styles_cache().await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}
