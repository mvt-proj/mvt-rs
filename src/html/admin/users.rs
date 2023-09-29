use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{get_auth, auth::{Auth, User}};

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewUser<'a> {
    username: &'a str,
    email: String,
    password: String,
}

#[handler]
pub async fn create_user<'a>(new_user: NewUser<'a>) -> Result<String, anyhow::Error> {
    let mut auth: Auth = get_auth().clone();
    let encrypt_psw = auth.get_encrypt_psw(new_user.password.to_string()).unwrap();
    let user = User {
        username: new_user.username.to_string(),
        email: new_user.email,
        password: encrypt_psw,
    };

    auth.users.push(user);

    let json_str = serde_json::to_string(&auth.users)?;
    let file_path = Path::new(&auth.config_dir).join("users.json");
    let mut file = File::create(file_path).await?;
    file.write_all(json_str.as_bytes()).await?;
    file.flush().await?;
    Ok(json_str)
}
