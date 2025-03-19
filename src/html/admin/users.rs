// use std::error::Error;
// use std::fmt;

use std::collections::HashMap;

use askama::Template;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{Group, User},
    config::users::create_user as create_cf_user,
    error::{AppError, AppResult},
    get_auth,
    html::main::{get_session_data, BaseTemplateData},
};

#[derive(Template)]
#[template(path = "admin/users/users.html")]
struct ListUsersTemplate<'a> {
    users: &'a Vec<User>,
    current_user: &'a User,
    base: BaseTemplateData,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    id: Option<String>,
    username: &'a str,
    email: String,
    password: String,
    groups: Vec<String>,
}

#[handler]
pub async fn list_users(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (is_auth, user) = get_session_data(depot).await;
    let auth = get_auth().await.read().await;

    let translate: HashMap<String, String> = HashMap::new();
    let base = BaseTemplateData { is_auth, translate };

    let current_user = user.unwrap();

    let template = ListUsersTemplate {
        users: &auth.users,
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_user<'a>(res: &mut Response, new_user: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;
    let encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string());

    if let Err(err) = encrypt_psw {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(AppError::PasswordHashError(err));
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
        password: encrypt_psw.unwrap(),
        groups: selected_groups,
    };

    if let Err(err) = create_cf_user(&user, None).await {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(AppError::SQLError(err));
    }

    auth.users.push(user);
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));
    Ok(())
}

#[handler]
pub async fn update_user<'a>(res: &mut Response, new_user: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;

    let encrypt_psw = if new_user.password.is_empty() {
        match auth.get_user_by_id(new_user.id.clone().unwrap().as_str()) {
            Some(user) => Ok(user.password.clone()),
            None => {
                res.status_code(StatusCode::NOT_FOUND);
                return Err(AppError::UserNotFound);
            }
        }
    } else {
        auth.get_encrypt_psw(new_user.password.to_string())
    };

    if let Err(err) = encrypt_psw {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(AppError::PasswordHashError(err));
    }

    let selected_groups: Vec<Group> = new_user
        .groups
        .iter()
        .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
        .collect();

    let user = User {
        id: new_user.id.unwrap(),
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw.unwrap(),
        groups: selected_groups,
    };

    if let Err(err) = auth.update_user(user).await {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));
    Ok(())
}

#[handler]
pub async fn delete_user<'a>(res: &mut Response, req: &mut Request) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;

    let id = req.param::<String>("id").ok_or_else(|| {
        res.status_code(StatusCode::BAD_REQUEST);
        AppError::RequestParamError("id".to_string())
    })?;

    if let Err(err) = auth.delete_user(id).await {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));
    Ok(())
}
