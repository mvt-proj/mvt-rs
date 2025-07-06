use base64::{Engine as _, engine::general_purpose};
use salvo::prelude::*;
use salvo::session::Session;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use time::{Duration, OffsetDateTime};

use jsonwebtoken::{self, EncodingKey};
use salvo::jwt_auth::{ConstDecoder, HeaderFinder};

use crate::config::groups::{create_group, delete_group, update_group};
use crate::config::users::{create_user, delete_user, get_users, update_user};
use crate::{
    config::groups::get_groups,
    error::{AppError, AppResult},
    get_auth, get_jwt_secret,
};
use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

fn decode_basic_auth(base64_string: &str) -> AppResult<(String, String)> {
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

    Ok((auth_parts[0].to_string(), auth_parts[1].to_string()))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub id: String,
    username: String,
    email: String,
    exp: i64,
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
                println!("Group not found");
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
    config_dir: String,
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
            if username == user.username && self.validate_psw(user, psw).unwrap() {
                return true;
            }
        }
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
            None => println!("user not found"),
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

    fn get_current_username_and_password(
        &self,
        authorization_str: &str,
    ) -> AppResult<(String, String)> {
        let (current_username, password) = match decode_basic_auth(authorization_str) {
            Ok(username) => username,
            Err(err) => {
                eprintln!("Error: {err}");
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

#[handler]
pub async fn require_user_admin(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth = get_auth().await.read().await;
            if let Some(user) = auth.get_user_by_id(&userid) {
                if !user.is_admin() {
                    res.render(Redirect::other("/admin"));
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

pub fn jwt_auth_handler() -> JwtAuth<JwtClaims, ConstDecoder> {
    let jwt_secret = get_jwt_secret();

    JwtAuth::new(ConstDecoder::from_secret(jwt_secret.as_bytes()))
        .finders(vec![Box::new(HeaderFinder::new())])
        .force_passed(true)
}

#[handler]
pub async fn login<'a>(res: &mut Response, depot: &mut Depot, data: Login<'a>) -> AppResult<()> {
    let auth = get_auth().await.read().await;

    let user = auth.get_user_by_email_and_password(data.email, data.password);

    if let Err(err) = user {
        res.status_code(StatusCode::UNAUTHORIZED);
        return Err(err);
    }

    let user = user?;

    let mut session = Session::new();
    session.insert("userid", user.id.clone()).unwrap();
    depot.set_session(session);

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin"));
    Ok(())
}

#[handler]
pub async fn logout(depot: &mut Depot, res: &mut Response) -> AppResult<()> {
    if let Some(session) = depot.session_mut() {
        session.remove("userid");
        // session.destroy();
    }
    res.render(Redirect::other("/"));
    Ok(())
}

#[handler]
pub async fn session_auth_handler(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    if let Some(session) = depot.session_mut() {
        if let Some(_userid) = session.get::<String>("userid") {
        } else {
            res.render(Redirect::other("/login"));
            return Ok(());
        }
    }

    Ok(())
}

#[handler]
pub async fn change_password<'a>(
    depot: &mut Depot,
    res: &mut Response,
    data: ChangePassword<'a>,
) -> AppResult<()> {
    let user_id = depot
        .session_mut()
        .and_then(|session| session.get::<String>("userid"))
        .ok_or(AppError::SessionNotFound);

    if let Err(err) = user_id {
        res.status_code(StatusCode::CONFLICT);
        return Err(err);
    }

    let user_id = user_id?;
    let mut auth = get_auth().await.write().await;

    let user = auth
        .get_user_by_id(&user_id)
        .ok_or(AppError::UserNotFoundError(user_id.clone()));

    if let Err(err) = user {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(err);
    }

    let mut user = user?.clone();
    let new_password = auth.get_encrypt_psw(data.password.to_string())?;
    user.password = new_password;
    auth.update_user(user).await?;

    res.render(Redirect::other("/"));

    Ok(())
}
