use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Group, User},
    error::{AppError, AppResult},
    get_auth,
    html::utils::{BaseTemplateData, make_base},
};

#[derive(Template)]
#[template(path = "admin/groups/groups.html")]
struct ListGroupsTemplate<'a> {
    groups: &'a Vec<Group>,
    current_user: &'a User,
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

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewGroup<'a> {
    id: Option<String>,
    name: &'a str,
    description: String,
}

#[handler]
pub async fn list_groups(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;
    let Some(current_user) = user else {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };
    let auth = get_auth().await.read().await;

    let template = ListGroupsTemplate {
        groups: &auth.groups,
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_group_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let template = NewGroupTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_group_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

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

#[handler]
pub async fn create_group<'a>(res: &mut Response, group_form: NewGroup<'a>) -> AppResult<()> {
    let group = Group::new(
        group_form.name.to_string(),
        group_form.description.to_string(),
    )
    .await;

    if let Err(err) = group {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    group?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/groups"));
    Ok(())
}

#[handler]
pub async fn update_group<'a>(res: &mut Response, group_form: NewGroup<'a>) -> AppResult<()> {
    let group_id = group_form
        .id
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let group = Group::from_id(&group_id).await;

    if let Err(err) = group {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let group = group?;

    if let Err(err) = group
        .update_group(
            group_form.name.to_string(),
            group_form.description.to_string(),
        )
        .await
    {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/groups"));
    Ok(())
}

#[handler]
pub async fn delete_group(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let group = Group::from_id(&id).await;

    if let Err(err) = group {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let group = group?;

    if let Err(err) = group.delete_group().await {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/groups"));
    Ok(())
}
