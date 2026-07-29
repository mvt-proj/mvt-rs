use salvo::http::StatusCode;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{AuthorizeState, DataToken, User},
    error::{AppError, AppResult},
    get_auth,
};

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: String,
    groups: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct LoginData<'a> {
    email: &'a str,
    password: String,
}

fn unauthorized(res: &mut Response) {
    let state = AuthorizeState {
        message: "Unauthorized".to_string(),
        status_code: 401,
    };
    res.status_code(StatusCode::UNAUTHORIZED);
    res.render(Json(&state));
}

#[handler]
pub async fn login<'a>(res: &mut Response, login_data: LoginData<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;
    let token = auth.login(login_data.email, &login_data.password)?;

    if token.is_empty() {
        unauthorized(res);
    } else {
        let data = DataToken { token };
        res.render(Json(&data));
    }
    Ok(())
}

#[handler]
pub async fn index(res: &mut Response) {
    let auth = get_auth().await.read().await;
    let users = &auth.users;
    res.render(Json(&users));
}

#[handler]
pub async fn create<'a>(res: &mut Response, data: NewUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;
    let encrypt_psw = auth
        .get_encrypt_psw(data.password.to_string())
        .map_err(AppError::PasswordHashError)?;

    let groups = data
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: data.username.to_string(),
        email: data.email,
        first_name: data.first_name,
        last_name: data.last_name,
        password: encrypt_psw,
        groups,
    };

    auth.create_user(user.clone()).await?;
    res.render(Json(&user));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateUser<'a> {
    #[salvo(extract(source(from = "param")))]
    id: String,
    username: &'a str,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    password: Option<String>,
    groups: Option<Vec<String>>,
}

#[handler]
pub async fn update<'a>(res: &mut Response, data: UpdateUser<'a>) -> AppResult<()> {
    let mut auth = get_auth().await.write().await;

    let existing = auth
        .get_user_by_id(&data.id)
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", data.id)))?
        .clone();

    let password =
        crate::auth::models::resolve_updated_password(&auth, &existing.password, data.password)?;

    let groups = data
        .groups
        .as_ref()
        .map(|names| auth.resolve_groups_by_name(names))
        .unwrap_or_default();

    let user = User {
        id: data.id,
        username: data.username.to_string(),
        email: data.email,
        first_name: data.first_name,
        last_name: data.last_name,
        password,
        groups,
    };

    auth.update_user(user.clone()).await?;
    res.render(Json(&user));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let mut auth = get_auth().await.write().await;
    if auth.get_user_by_id(&id).is_none() {
        return Err(AppError::NotFound(format!("User {id} not found")));
    }

    auth.delete_user(id).await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
