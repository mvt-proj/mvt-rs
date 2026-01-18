use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use jsonwebtoken::EncodingKey;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use time::{Duration, OffsetDateTime};
use tracing:: warn;

use crate::config::groups::{create_group, delete_group, get_groups, update_group};
use crate::config::users::{create_user, delete_user, get_users, update_user};
use crate::error::{AppError, AppResult};
use crate::{get_auth, get_jwt_secret};

use super::utils::decode_basic_auth;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub id: String,
    pub username: String,
    pub email: String,
    pub exp: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthorizeState {
    pub message: String,
    pub status_code: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl Group {
    pub async fn new(name: String, description: String) -> AppResult<Self> {
        let group = Group {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
        };

        create_group(&group, None).await?;
        let mut auth = get_auth().await.write().await;
        auth.groups.push(group.clone());
        Ok(group)
    }

    pub async fn from_id(id: &str) -> AppResult<Self> {
        let auth = get_auth().await.read().await;
        let group = auth.groups.iter().find(|group| group.id == id).unwrap();
        Ok(group.clone())
    }

    pub async fn update_group(&self, name: String, description: String) -> AppResult<Self> {
        let group = Group {
            id: self.id.clone(),
            name,
            description,
        };

        update_group(self.id.clone(), &group, None).await?;
        let mut auth = get_auth().await.write().await;

        let position = auth.groups.iter().position(|group| group.id == self.id);

        match position {
            Some(pos) => {
                auth.groups[pos] = group.clone();
            }
            None => {
                auth.groups.push(group.clone());
            }
        }

        Ok(group)
    }

    pub async fn delete_group(&self) -> AppResult<()> {
        let mut auth = get_auth().await.write().await;
        let position = auth.groups.iter().position(|group| group.id == self.id);

        delete_group(self.id.clone(), None).await?;

        match position {
            Some(pos) => {
                auth.groups.remove(pos);
            }
            None => {
                warn!(group_id = %self.id, "Group not found during deletion");
            }
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
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

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
pub struct Login<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
pub struct ChangePassword<'a> {
    pub password: &'a str,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Auth {
    pub groups: Vec<Group>,
    pub users: Vec<User>,
    pub config_dir: String,
}

impl Auth {
    pub async fn new(config_dir: &str, pool: &SqlitePool) -> AppResult<Self> {
        let groups = get_groups(Some(pool)).await?;
        let users = get_users(Some(pool)).await?;

        Ok(Self {
            groups,
            users,
            config_dir: config_dir.to_string(),
        })
    }

    pub fn find_group_by_name<'a>(&'a self, target_name: &'a str) -> Option<&'a Group> {
        self.groups.iter().find(|m| m.name == target_name)
    }

    pub fn get_encrypt_psw(&self, psw: String) -> Result<String, argon2::password_hash::Error> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(psw.as_bytes(), &salt)?.to_string();
        Ok(password_hash)
    }

    fn validate_psw(&self, user: User, psw: &str) -> AppResult<bool> {
        let parsed_hash = PasswordHash::new(&user.password)?;
        Argon2::default().verify_password(psw.as_bytes(), &parsed_hash)?;
        Ok(true)
    }

    pub fn validate_user(&mut self, username: &str, psw: &str) -> bool {
        for user in self.users.clone().into_iter() {
            if username == user.username {
                // Manejar el error de validación de password correctamente
                match self.validate_psw(user, psw) {
                    Ok(true) => return true,
                    Ok(false) | Err(_) => {
                        warn!(username = %username, "Invalid password attempt");
                        return false;
                    }
                }
            }
        }
        warn!(username = %username, "User not found");
        false
    }

    pub fn get_user_by_authorization(&mut self, authorization: &str) -> AppResult<Option<&User>> {
        let user = self.get_current_username_and_password(authorization)?;
        if self.validate_user(&user.0, &user.1) {
            return Ok(self.find_user_by_name(&user.0));
        }
        Ok(None)
    }

    pub async fn create_user(&mut self, user: User) -> AppResult<User> {
        self.users.push(user.clone());
        create_user(&user, None).await?;
        Ok(user)
    }

    pub async fn update_user(&mut self, user: User) -> AppResult<()> {
        let id = user.id.clone();
        update_user(id, &user, None).await?;
        let position = self.users.iter().position(|usr| usr.id == user.id);
        match position {
            Some(index) => self.users[index] = user,
            None => warn!(user_id = %user.id, "User not found during update"),
        }
        Ok(())
    }

    pub async fn delete_user(&mut self, id: String) -> AppResult<()> {
        delete_user(id.clone(), None).await?;
        self.users.retain(|user| user.id != id);
        Ok(())
    }

    pub fn find_user_by_name<'a>(&'a self, target_name: &str) -> Option<&'a User> {
        self.users.iter().find(|usr| usr.username == target_name)
    }

    pub fn get_user_by_id<'a>(&'a self, target_id: &str) -> Option<&'a User> {
        self.users.iter().find(|usr| usr.id == target_id)
    }

    pub fn find_user_position_by_name(&self, target_name: &str) -> Option<usize> {
        self.users
            .iter()
            .position(|usr| usr.username == target_name)
    }

    pub fn get_current_username_and_password(
        &self,
        authorization_str: &str,
    ) -> AppResult<(String, String)> {
        let (current_username, password) = match decode_basic_auth(authorization_str) {
            Ok(username) => username,
            Err(err) => {
                warn!(error = %err, "Failed to decode basic auth");
                return Ok((String::new(), String::new()));
            }
        };
        Ok((current_username, password))
    }

    pub fn login(&mut self, email: &str, psw: &str) -> AppResult<String> {
        let jwt_secret = get_jwt_secret();
        for user in self.users.clone().into_iter() {
            if email == user.email && self.validate_psw(user.clone(), psw)? {
                let exp = OffsetDateTime::now_utc() + Duration::days(1);
                let claim = JwtClaims {
                    id: user.id,
                    username: user.username.to_owned(),
                    email: email.to_owned(),
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

    pub fn get_user_by_email_and_password(&self, email: &str, password: &str) -> AppResult<User> {
        self.users
            .iter()
            .find(|user| {
                user.email == email
                    && self
                        .validate_psw((*user).clone(), password)
                        .unwrap_or(false)
            })
            .cloned()
            .ok_or(AppError::UserNotFound)
    }
}
