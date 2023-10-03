use salvo::macros::Extractible;
use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    auth::{Auth, User},
    get_app_state,
    get_auth,
};

#[derive(Template)]
#[template(path = "admin/users.html")]
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
pub async fn list_users(res: &mut Response) {
    let auth: Auth = get_auth().clone();
    // let _ = auth.refresh().await;
    let template = ListUsersTemplate {
        users: &auth.users,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn create_user<'a>(res: &mut Response, new_user: NewUser<'a>) {
    let auth: Auth = get_auth().clone();
    let app_state = get_app_state();
    let encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string()).unwrap();
    let user = User {
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw,
    };

    app_state.auth.users.push(user);

    let json_str = serde_json::to_string(&auth.users).unwrap();
    let file_path = Path::new(&auth.config_dir).join("users.json");
    let mut file = File::create(file_path).await.unwrap();
    file.write_all(json_str.as_bytes()).await.unwrap();
    file.flush().await.unwrap();
    res.render(Redirect::other("/admin/users"));

    // Ok(json_str)
}
