// use std::error::Error;
// use std::fmt;

use askama::Template;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{Auth, Group, User}, config::create_user as create_cf_user, error::{AppError, AppResult}, get_app_state, get_auth
};

#[derive(Template)]
#[template(path = "admin/users/users.html")]
struct ListUsersTemplate<'a> {
    users: &'a Vec<User>,
    current_user: &'a User,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    password: String,
    groups: Vec<String>,
}

#[handler]
pub async fn list_users(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let authorization = req.headers().get("authorization").unwrap(); //.ok_or(AppError::ParseHeaderError);
    let authorization_str = authorization
        .to_str()
        .map_err(|err| AppError::ConversionError(err.to_string()))?;

    let auth: Auth = get_auth().clone();
    let current_user = auth.get_current_user(authorization_str).unwrap();

    let template = ListUsersTemplate {
        users: &auth.users,
        current_user,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_user<'a>(res: &mut Response, new_user: NewUser<'a>) -> AppResult<()> {
    let auth: Auth = get_auth().clone();
    let app_state = get_app_state();
    let encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string())?;

    let selected_groups: Vec<Group> = new_user
        .groups
        .iter()
        .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
        .collect();

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw,
        groups: selected_groups,
    };

    create_cf_user(&user, None).await?;

    app_state.auth.users.push(user);

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));
    Ok(())
}

#[handler]
pub async fn update_user<'a>(res: &mut Response, new_user: NewUser<'a>) -> AppResult<()> {
    let auth: Auth = get_auth().clone();
    let app_state = get_app_state();

    let encrypt_psw: String;
    if new_user.password.to_string().is_empty() {
        match auth.find_user_by_name(new_user.username) {
            Some(user) => encrypt_psw = user.password.clone(),
            None => encrypt_psw = "".to_string(),
        };
    } else {
        encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string())?;
    }

    let selected_groups: Vec<Group> = new_user
        .groups
        .iter()
        .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
        .collect();

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw,
        groups: selected_groups,
    };
    let _ = app_state.auth.update_user(user).await;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));
    Ok(())
}

#[handler]
pub async fn delete_user<'a>(res: &mut Response, req: &mut Request) -> AppResult<()> {
    let app_state = get_app_state();

    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    app_state.auth.delete_user(id).await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));

    Ok(())
}
