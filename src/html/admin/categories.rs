use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::User,
    error::{AppError, AppResult},
    get_categories,
    html::utils::{BaseTemplateData, make_base},
    models::category::Category,
};

#[derive(Template)]
#[template(path = "admin/categories/categories.html")]
struct ListCategoriesTemplate<'a> {
    categories: &'a Vec<Category>,
    current_user: &'a User,
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

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewCategory<'a> {
    id: Option<String>,
    name: &'a str,
    description: String,
}

#[handler]
pub async fn list_categories(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;
    let Some(current_user) = user else {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };

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
pub async fn new_category_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let template = NewCategoryTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_category_page(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

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
pub async fn create_category<'a>(
    res: &mut Response,
    category_form: NewCategory<'a>,
) -> AppResult<()> {
    let category = Category::new(
        category_form.name.to_string(),
        category_form.description.to_string(),
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
pub async fn update_category<'a>(
    res: &mut Response,
    category_form: NewCategory<'a>,
) -> AppResult<()> {
    let category_id = category_form
        .id
        .as_ref()
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let category = Category::from_id(category_id).await;

    if let Err(err) = category {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let updated_category = category?
        .update_category(
            category_form.name.to_string(),
            category_form.description.to_string(),
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
