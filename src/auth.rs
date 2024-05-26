use salvo::basic_auth::{BasicAuth, BasicAuthValidator};
use base64::{engine::general_purpose, Engine as _};
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use jsonwebtoken::{self, EncodingKey};
use salvo::jwt_auth::{ConstDecoder, HeaderFinder};

use crate::{error::{AppResult, AppError}, get_auth, get_jwt_secret, storage::Storage};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};


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
pub struct Group {
    pub name: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub username: String,
    pub email: String,
    // #[serde(skip_serializing)]
    pub password: String,
    pub groups: Vec<Group>,
}

impl User {
    pub fn groups_as_string(&self) -> String {
        self.groups
            .iter()
            .map(|group| group.name.clone())
            .collect::<Vec<String>>()
            .join(" | ")
    }

    pub fn groups_as_vec_string(&self) -> Vec<String> {
        self.groups
            .iter()
            .map(|group| group.name.clone())
            .collect::<Vec<String>>()
    }

    pub fn is_admin(&self) -> bool {
        self.groups_as_vec_string().contains(&"admin".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DataToken {
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Auth {
    pub groups: Vec<Group>,
    pub users: Vec<User>,
    config_dir: String,
    pub groups_path: String,
    pub users_path: String,
    salt_string: String,
}

impl Auth {
    pub async fn new(config_dir: &str, salt_string: String) -> AppResult<Self> {
        // Groups
        let groups_path = format!("{config_dir}/groups.json");
        let mut storage = Storage::<Vec<Group>>::new(groups_path.clone());
        let loaded_groups = storage.load().await?;
        let mut groups: Vec<Group> = loaded_groups.unwrap_or(Vec::new());

        if groups.is_empty() {
            let admin_group = Group {
                name: "admin".to_string(),
                description: "admin role".to_string(),
            };
            let operator_group = Group {
                name: "operator".to_string(),
                description: "operator role".to_string(),
            };
            groups.push(admin_group);
            groups.push(operator_group);
            storage.save(groups.clone()).await?;
        }

        // Users
        let users_path = format!("{config_dir}/users.json");
        let mut storage = Storage::<Vec<User>>::new(users_path.clone());
        let loaded_users = storage.load().await?;
        let mut users: Vec<User> = loaded_users.unwrap_or(Vec::new());

        if users.is_empty() {
            let salt = SaltString::encode_b64(salt_string.as_bytes())?;
            let argon2 = Argon2::default();
            let password_hash = argon2
                .hash_password("admin".to_string().as_bytes(), &salt)
                .unwrap()
                .to_string();
            let admin_group = Group {
                name: "admin".to_string(),
                description: "admin role".to_string(),
            };
            let user = User {
                username: "admin".to_string(),
                email: "admin@mail.com".to_string(),
                password: password_hash,
                groups: vec![admin_group],
            };
            users.push(user);
            storage.save(users.clone()).await?;
        }


        Ok(Self {
            groups,
            users,
            config_dir: config_dir.to_string(),
            groups_path,
            users_path,
            salt_string,
        })
    }


    pub fn find_group_by_name<'a>(&'a self, target_name: &'a str) -> Option<&'a Group> {
        self.groups.iter().find(|m| m.name == target_name)
    }

    pub fn get_encrypt_psw(&self, psw: String) -> Result<String, argon2::password_hash::Error> {
        let salt = SaltString::encode_b64(self.salt_string.as_bytes())?;
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(psw.as_bytes(), &salt)?.to_string();
        Ok(password_hash)
    }

    fn validate_psw(&self, user: User, psw: &str) -> AppResult<bool> {
        let salt = SaltString::encode_b64(self.salt_string.as_bytes())?;
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(psw.as_bytes(), &salt)?.to_string();
        Ok(password_hash == user.password)
    }

    pub fn validate_user(&mut self, username: &str, psw: &str) -> bool {
        for user in self.users.clone().into_iter() {
            if username == user.username && self.validate_psw(user, psw).unwrap() {
                return true;
            }
        }
        false
    }

    pub async fn create_user(&mut self, user: User) -> AppResult<User> {
        self.users.push(user.clone());
        let mut storage = Storage::<Vec<User>>::new(self.users_path.clone());

        storage.save(self.users.clone()).await?;
        Ok(user)
    }

    pub async fn update_user(&mut self, user: User) -> AppResult<()> {
        let position = self
            .users
            .iter()
            .position(|usr| usr.username == user.username);
        match position {
            Some(index) => self.users[index] = user,
            None => println!("user not found"),
        }
        let mut storage = Storage::<Vec<User>>::new(self.users_path.clone());
        storage.save(self.users.clone()).await?;
        Ok(())
    }

    pub async fn delete_user(&mut self, username: String) -> AppResult<()> {
        self.users.retain(|user| user.username != username);
        let mut storage = Storage::<Vec<User>>::new(self.users_path.clone());
        storage.save(self.users.clone()).await?;
        Ok(())
    }

    pub fn find_user_by_name<'a>(&'a self, target_name: &str) -> Option<&'a User> {
        self.users.iter().find(|usr| usr.username == target_name)
    }

    pub fn find_user_position_by_name(&self, target_name: &str) -> Option<usize> {
        self.users
            .iter()
            .position(|usr| usr.username == target_name)
    }

    pub fn get_current_user(&self, authorization_str: &str) -> Option<&User> {
        let current_username = match decode_basic_auth(authorization_str) {
            Ok(username) => username,
            Err(err) => {
                eprintln!("Error: {}", err);
                return None;             }
        };
        self.find_user_by_name(&current_username)
    }

    pub fn login(&mut self, username: &str, psw: &str) -> AppResult<String> {
        let jwt_secret = get_jwt_secret();
        for user in self.users.clone().into_iter() {
            if username == user.username && self.validate_psw(user, psw)? {
                let exp = OffsetDateTime::now_utc() + Duration::days(14);
                let claim = JwtClaims {
                    username: username.to_owned(),
                    exp: exp.unix_timestamp(),
                };
                let token = jsonwebtoken::encode(
                    &jsonwebtoken::Header::default(),
                    &claim,
                    &EncodingKey::from_secret(jwt_secret.as_bytes()),
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

pub fn jwt_auth_handler() -> JwtAuth<JwtClaims, ConstDecoder> {
    let jwt_secret = get_jwt_secret();

    JwtAuth::new(ConstDecoder::from_secret(jwt_secret.as_bytes()))
        .finders(vec![Box::new(HeaderFinder::new())])
        .force_passed(true)
}

pub struct Validator;
impl BasicAuthValidator for Validator {
    async fn validate(&self, username: &str, password: &str, _depot: &mut Depot) -> bool {
        let mut auth: Auth = get_auth().clone();
        auth.validate_user(username, password)
    }
}

pub fn basic_auth_handler() -> BasicAuth<Validator> {
    BasicAuth::new(Validator)
}
