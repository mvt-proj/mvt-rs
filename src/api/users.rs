use salvo::http::StatusCode;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, AuthorizeState, DataToken, User},
    get_app_state, get_auth,
};

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct LoginData<'a> {
    username: &'a str,
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
    let mut auth: Auth = get_auth().clone();
    let token = auth
        .login(login_data.username, &login_data.password)
        .unwrap();

    if token.is_empty() {
        unauthorized(res);
    } else {
        let data = DataToken { token };
        res.render(Json(&data));
    }
}

#[handler]
pub async fn index(res: &mut Response) {
    let auth: Auth = get_auth().clone();
    let users = auth.users;
    res.render(Json(&users));
}

#[handler]
pub async fn create<'a>(res: &mut Response, data: NewUser<'a>) {
    let auth: Auth = get_auth().clone();
    let app_state = get_app_state();
    let encrypt_psw = auth.get_encrypt_psw(data.password.to_string()).unwrap();
    let user = User {
        username: data.username.to_string(),
        email: data.email,
        password: encrypt_psw,
    };

    app_state.auth.create_user(user.clone()).await.unwrap();
    res.render(Json(&user));
}
