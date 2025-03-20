use std::collections::HashMap;

use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Group, User},
    error::{AppError, AppResult},
    get_auth,
    html::main::{get_session_data, BaseTemplateData},
};

#[derive(Template)]
#[template(path = "admin/groups/groups.html")]
struct ListGroupsTemplate<'a> {
    groups: &'a Vec<Group>,
    current_user: &'a User,
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
    let (is_auth, user) = get_session_data(depot).await;

    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };
    let current_user = user.unwrap();
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
pub async fn create_group<'a>(res: &mut Response, new_group: NewGroup<'a>) -> AppResult<()> {
    let group = Group::new(
        new_group.name.to_string(),
        new_group.description.to_string(),
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
pub async fn edit_group<'a>(res: &mut Response, new_group: NewGroup<'a>) -> AppResult<()> {
    let group = Group::from_id(&new_group.id.unwrap()).await;

    if let Err(err) = group {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let group = group?;

    if let Err(err) = group
        .update_group(
            new_group.name.to_string(),
            new_group.description.to_string(),
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
