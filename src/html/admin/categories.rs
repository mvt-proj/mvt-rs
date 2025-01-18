use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, User},
    error::{AppError, AppResult},
    get_auth,
    html::main::BaseTemplateData,
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
    let app_state = crate::get_app_state();

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

    let template = ListCategoriesTemplate {
        categories: &app_state.categories,
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
