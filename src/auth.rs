use salvo::basic_auth::BasicAuthValidator;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use time::{Duration, OffsetDateTime};

use salvo::jwt_auth::{ConstDecoder, HeaderFinder};
use jsonwebtoken::{self, EncodingKey};

use crate::get_auth;
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};

const SECRET_KEY: &str = "YOUR SECRET_KEY";

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    username: String,
    exp: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthorizeState {
    pub message: String,
    pub status_code: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub username: String,
    pub email: String,
    // #[serde(skip_serializing)]
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DataToken {
    pub token: String,
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

        let mut users: Vec<User>;

        if file_path.exists() {
            let mut file = File::open(file_path).await?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).await?;
            users = serde_json::from_str(&contents.clone())?;
        } else {
            users = Vec::new();
            let salt = SaltString::encode_b64(salt_string.as_bytes())
                .unwrap();
            let argon2 = Argon2::default();
            let password_hash = argon2
                .hash_password("admin".to_string().as_bytes(), &salt)
                .unwrap()
                .to_string();
            let user = User{
                username: "admin".to_string(),
                email: "admin@mail.com".to_string(),
                password: password_hash,
            };
            users.push(user);

            let json_str = serde_json::to_string(&users)?;
            let file_path = Path::new(config_dir).join("users.json");
            let mut file = File::create(file_path).await?;
            file.write_all(json_str.as_bytes()).await?;
            file.flush().await?;
        }

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

    pub fn validate_user(&mut self, username: &str, psw: &str) -> bool {
        // let _ = self.refresh();
        for user in self.users.clone().into_iter() {
            if username == user.username && self.validate_psw(user, psw).unwrap() {
                return true;
            }
        }
        false
    }

    pub async fn create_user(&mut self, user: User) -> Result<User, anyhow::Error> {
        // user.password = self.get_encrypt_psw(&user.password);
        self.users.push(user.clone());

        let json_str = serde_json::to_string(&self.users)?;
        let file_path = Path::new(&self.config_dir).join("users.json");
        let mut file = File::create(file_path).await?;
        file.write_all(json_str.as_bytes()).await?;
        file.flush().await?;
        Ok(user)

    }

    pub fn login(&mut self, username: &str, psw: &str) -> Result<String, anyhow::Error> {
        for user in self.users.clone().into_iter() {
            if username == user.username && self.validate_psw(user, psw).unwrap() {
                let exp = OffsetDateTime::now_utc() + Duration::days(14);
                let claim = JwtClaims {
                    username: username.to_owned(),
                    exp: exp.unix_timestamp(),
                };
                let token = jsonwebtoken::encode(
                    &jsonwebtoken::Header::default(),
                    &claim,
                    &EncodingKey::from_secret(SECRET_KEY.as_bytes()),
                )?;
                return Ok(token);
            }
        }
        Ok("".to_owned())
    }
}


#[handler]
pub async fn validate_token(depot: &mut Depot, res: &mut Response) {
    match depot.jwt_auth_state() {
        JwtAuthState::Authorized => {

            // let token = depot.jwt_auth_token().unwrap();
            // println!("TOKEN: {}", token);
        }
        JwtAuthState::Unauthorized => {
            let state = AuthorizeState {
                message: "Unauthorized".to_string(),
                status_code: 401,
            };
            res.status_code(StatusCode::from_u16(401).unwrap());
            res.render(Json(&state));
        }
        JwtAuthState::Forbidden => {
            let state = AuthorizeState {
                message: "Forbidden".to_string(),
                status_code: 403,
            };
            res.status_code(StatusCode::from_u16(403).unwrap());
            res.render(Json(&state));
        }
    }
}

// pub fn auth_handler(secret_key: String) -> JwtAuth<JwtClaims, ConstDecoder> {
pub fn auth_handler() -> JwtAuth<JwtClaims, ConstDecoder> {

    JwtAuth::new(ConstDecoder::from_secret(SECRET_KEY.as_bytes()))
        .finders(vec![
            Box::new(HeaderFinder::new()),
        ])
        .force_passed(true)
}




pub struct Validator;
#[async_trait]
impl BasicAuthValidator for Validator {
    async fn validate(&self, username: &str, password: &str, _depot: &mut Depot) -> bool {
        let mut auth: Auth = get_auth().clone();
        auth.validate_user(username, password)
    }
}
