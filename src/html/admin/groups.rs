use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, Group, User},
    error::{AppError, AppResult},
    get_auth,
    html::main::BaseTemplateData,
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

    let template = ListGroupsTemplate {
        groups: &app_state.auth.groups,
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_group<'a>(res: &mut Response, new_group: NewGroup<'a>) -> AppResult<()> {
    Group::new(
        new_group.name.to_string(),
        new_group.description.to_string(),
    )
    .await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/groups"));
    Ok(())
}

#[handler]
pub async fn edit_group<'a>(res: &mut Response, new_group: NewGroup<'a>) -> AppResult<()> {
    let group = Group::from_id(&new_group.id.unwrap()).await?;

    group
        .update_group(
            new_group.name.to_string(),
            new_group.description.to_string(),
        )
        .await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/groups"));
    Ok(())
}

#[handler]
pub async fn delete_group(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;

    let group = Group::from_id(&id).await?;
    group.delete_group().await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/groups"));
    Ok(())
}
