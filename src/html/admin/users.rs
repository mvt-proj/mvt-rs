use askama::Template;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Auth, User},
    get_app_state, get_auth,
    storage::Storage,
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
    let template = ListUsersTemplate { users: &auth.users };
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

    let mut storage = Storage::<Vec<User>>::new(auth.storage_path.clone());
    storage.save(app_state.auth.users.clone()).await.unwrap();
    res.render(Redirect::other("/admin/users"));
}

#[handler]
pub async fn delete_user<'a>(res: &mut Response, req: &mut Request) {
    let app_state = get_app_state();

    let username = req.param::<String>("username").unwrap();
    app_state.auth.delete_user(username).await.unwrap();
    res.render(Redirect::other("/admin/users"));
}
