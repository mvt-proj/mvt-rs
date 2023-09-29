use salvo::basic_auth::BasicAuthValidator;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::get_auth;
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Auth {
    pub users: Vec<User>,
    pub config_dir: String,
    salt_string: String,
}

impl Auth {
    pub async fn new(config_dir: &str, salt_string: String) -> Result<Self, anyhow::Error> {
        let file_path = Path::new(config_dir).join("users.json".to_string());
        let mut file = File::open(file_path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;

        let users: Vec<User> = serde_json::from_str(&contents.clone())?;
        Ok(Self {
            users,
            config_dir: config_dir.to_string(),
            salt_string,
        })
    }

    pub fn get_encrypt_psw(&self, psw: String) -> Result<String, argon2::password_hash::Error> {
        let salt = SaltString::encode_b64(self.salt_string.as_bytes())?;
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(psw.as_bytes(), &salt)?.to_string();
        Ok(password_hash)
    }

    fn validate_psw(&self, user: User, psw: &str) -> Result<bool, argon2::password_hash::Error> {
        let salt = SaltString::encode_b64(self.salt_string.as_bytes())?;
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(psw.as_bytes(), &salt)?.to_string();
        Ok(password_hash == user.password)
    }

    fn validate_user(&self, username: &str, psw: &str) -> bool {
        for user in self.users.clone().into_iter() {
            if username == user.username && self.validate_psw(user, psw).unwrap() {
                return true;
            }
        }
        false
    }
}

pub struct Validator;
#[async_trait]
impl BasicAuthValidator for Validator {
    async fn validate(&self, username: &str, password: &str, _depot: &mut Depot) -> bool {
        let auth: Auth = get_auth().clone();
        auth.validate_user(username, password)
    }
}
