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
    html::utils::{BaseTemplateData, get_session_data, is_authenticated},
};

// --- Templates ---
#[derive(Template)]
#[template(path = "admin/users/users.html")]
struct ListUsersTemplate<'a> {
    users: &'a Vec<User>,
    current_user: &'a User,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/users/new.html")]
struct NewUserTemplate {
    groups: Vec<Group>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/users/edit.html")]
struct EditUserTemplate {
    user: User,
    groups: Vec<Group>,
    base: BaseTemplateData,
}

// --- Structs de Datos ---
#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
pub struct NewUser<'a> {
    id: Option<String>,
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: String,
    groups: Vec<String>,
}

// --- Handlers ---

#[handler]
pub async fn list_users(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (is_auth, user) = get_session_data(depot).await;
    let auth = get_auth().await.read().await;

    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    if let Some(current_user) = user {
        let template = ListUsersTemplate {
            users: &auth.users,
            current_user: &current_user,
            base,
        };
        res.render(Text::Html(template.render()?));
        Ok(())
    } else {
        res.render(Redirect::other("/login"));
        Ok(())
    }
}

#[handler]
pub async fn new_user(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let auth = get_auth().await.read().await;
    let template = NewUserTemplate {
        groups: auth.groups.clone(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_user(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let auth = get_auth().await.read().await;
    let user = auth
        .get_user_by_id(&id)
        .ok_or(AppError::NotFound("User not found".into()))?
        .clone();

    let template = EditUserTemplate {
        user,
        groups: auth.groups.clone(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_user<'a>(res: &mut Response, user_form: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;
    let encrypt_psw = auth.get_encrypt_psw(user_form.password.to_string());

    if let Err(err) = encrypt_psw {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(AppError::PasswordHashError(err));
    }

    let selected_groups: Vec<Group> = user_form
        .groups
        .iter()
        .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
        .collect();

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: user_form.username.to_string(),
        email: user_form.email,
        password: encrypt_psw.unwrap(),
        groups: selected_groups,
        first_name: user_form.first_name.filter(|s| !s.is_empty()),
        last_name: user_form.last_name.filter(|s| !s.is_empty()),
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
pub async fn update_user<'a>(res: &mut Response, user_form: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;

    let encrypt_psw = if user_form.password.is_empty() {
        match auth.get_user_by_id(user_form.id.clone().unwrap().as_str()) {
            Some(user) => Ok(user.password.clone()),
            None => {
                res.status_code(StatusCode::NOT_FOUND);
                return Err(AppError::UserNotFound);
            }
        }
    } else {
        auth.get_encrypt_psw(user_form.password.to_string())
    };

    if let Err(err) = encrypt_psw {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(AppError::PasswordHashError(err));
    }

    let selected_groups: Vec<Group> = user_form
        .groups
        .iter()
        .filter_map(|group_name| auth.find_group_by_name(group_name).cloned())
        .collect();

    let user = User {
        id: user_form.id.unwrap(),
        username: user_form.username.to_string(),
        email: user_form.email,
        first_name: user_form.first_name.filter(|s| !s.is_empty()),
        last_name: user_form.last_name.filter(|s| !s.is_empty()),
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
