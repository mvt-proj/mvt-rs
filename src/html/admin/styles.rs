use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, User},
    error::{AppError, AppResult},
    get_auth,
    html::main::BaseTemplateData,
    models::{category::Category, styles::Style},
};

#[derive(Template)]
#[template(path = "admin/styles/styles.html")]
struct ListCategoriesTemplate<'a> {
    styles: &'a Vec<Style>,
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

    let styles = Style::get_all_styles().await?;
    let template = ListCategoriesTemplate {
        styles: &styles,
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_style<'a>(res: &mut Response, new_style: NewStyle<'a>) -> AppResult<()> {
    Style::new(
        new_style.name.to_string(),
        Category::from_id(&new_style.category).await?,
        new_style.description.to_string(),
        new_style.style.to_string(),
    )
    .await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn edit_style<'a>(res: &mut Response, new_style: NewStyle<'a>) -> AppResult<()> {
    let style = Style::from_id(&new_style.id.unwrap()).await?;
    style
        .update_style(
            new_style.name.to_string(),
            Category::from_id(&new_style.category).await?,
            new_style.description.to_string(),
            new_style.style.to_string(),
        )
        .await?;

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
