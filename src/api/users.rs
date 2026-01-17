use salvo::http::StatusCode;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{AuthorizeState, DataToken, Group, User},
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
    groups: Vec<Option<Group>>,
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
pub async fn login<'a>(res: &mut Response, login_data: LoginData<'a>) {
    let mut auth = get_auth().await.write().await;
    let token = auth.login(login_data.email, &login_data.password).unwrap();

    if token.is_empty() {
        unauthorized(res);
    } else {
        let data = DataToken { token };
        res.render(Json(&data));
    }
}

#[handler]
pub async fn index(res: &mut Response) {
    let auth = get_auth().await.read().await;
    let users = &auth.users;
    res.render(Json(&users));
}

#[handler]
pub async fn create<'a>(res: &mut Response, data: NewUser<'a>) {
    let mut auth = get_auth().await.write().await;
    let encrypt_psw = auth.get_encrypt_psw(data.password.to_string()).unwrap();

    let user = User {
        id: Uuid::new_v4().to_string(),
        username: data.username.to_string(),
        email: data.email,
        first_name: data.first_name,
        last_name: data.last_name,
        password: encrypt_psw,
        groups: Vec::new(),
    };

    auth.create_user(user.clone()).await.unwrap();
    res.render(Json(&user));
}
