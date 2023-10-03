use serde::{Deserialize, Serialize};
use salvo::macros::Extractible;
use salvo::http::StatusCode;
use salvo::prelude::*;

use crate::{
    auth::Auth,
    get_auth,
};


#[derive(Debug, Serialize)]
pub struct AuthorizeState {
    message: String,
    status_code: u16,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body", format = "json")))]
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

// pub async fn login (res: &mut Response) {
#[handler]
pub async fn login<'a>(res: &mut Response, login_data: LoginData<'a>) {

    let mut auth: Auth = get_auth().clone();
    let token = auth.login(&login_data.username, &login_data.password).unwrap();

    if token.is_empty() {
        unauthorized(res);
    } else {
        res.render(Json(&token));
    }
}
