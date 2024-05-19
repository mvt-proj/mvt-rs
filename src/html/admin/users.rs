use base64::{engine::general_purpose, Engine as _};
// use std::error::Error;
// use std::fmt;

use askama::Template;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, User},
    error::{AppError, AppResult},
    get_app_state, get_auth,
    storage::Storage,
};

// #[derive(Debug)]
// struct AuthError {
//     message: String,
// }
//
// impl fmt::Display for AuthError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "Authentication error: {}", self.message)
//     }
// }
//
// impl Error for AuthError {}

fn decode_basic_auth(base64_string: &str) -> AppResult<String> {
    let parts: Vec<&str> = base64_string.splitn(2, ' ').collect();

    if parts.len() != 2 || parts[0] != "Basic" {
        return Err(AppError::BasicAuthError(
            "Invalid Basic Authentication format".to_string(),
        ));
    }

    let decoded_bytes = general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| AppError::BasicAuthError("Failed to decode Base64".to_string()))?;

    let decoded_str = String::from_utf8(decoded_bytes)
        .map_err(|_| AppError::BasicAuthError("Failed to convert to UTF-8".to_string()))?;

    let auth_parts: Vec<&str> = decoded_str.splitn(2, ':').collect();

    if auth_parts.len() != 2 {
        return Err(AppError::BasicAuthError(
            "Invalid username:password format".to_string(),
        ));
    }

    Ok(auth_parts[0].to_string())
}

#[derive(Template)]
#[template(path = "admin/users/users.html")]
struct ListUsersTemplate<'a> {
    users: &'a Vec<User>,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    password: String,
}

#[handler]
pub async fn list_users(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let authorization = req.headers().get("authorization").unwrap(); //.ok_or(AppError::ParseHeaderError);
    let authorization_str = authorization
        .to_str()
        .map_err(|err| AppError::ConversionError(err.to_string()))?;
    let _username = match decode_basic_auth(authorization_str) {
        Ok(username) => username,
        Err(err) => {
            eprintln!("Error: {}", err);
            String::new()
        }
    };

    let auth: Auth = get_auth().clone();
    let template = ListUsersTemplate { users: &auth.users };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_user<'a>(res: &mut Response, new_user: NewUser<'a>) -> AppResult<()> {
    let auth: Auth = get_auth().clone();
    let app_state = get_app_state();
    let encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string())?;
    let user = User {
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw,
    };

    app_state.auth.users.push(user);

    let mut storage = Storage::<Vec<User>>::new(auth.storage_path.clone());
    storage.save(app_state.auth.users.clone()).await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));
    Ok(())
}

#[handler]
pub async fn update_user<'a>(res: &mut Response, new_user: NewUser<'a>) -> AppResult<()> {
    let auth: Auth = get_auth().clone();
    let app_state = get_app_state();
    let encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string())?;
    let user = User {
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw,
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

    let username = req
        .param::<String>("username")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    app_state.auth.delete_user(username).await?;
    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/users"));

    Ok(())
}
