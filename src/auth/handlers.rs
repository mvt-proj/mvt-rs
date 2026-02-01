use salvo::jwt_auth::{ConstDecoder, HeaderFinder, JwtAuth};
use salvo::prelude::*;
use salvo::session::Session;

use crate::error::{AppError, AppResult};
use crate::{get_auth, get_jwt_secret};

use super::models::{AuthorizeState, ChangePassword, JwtClaims, Login};

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
    if let Some(session) = depot.session_mut()
        && let Some(userid) = session.get::<String>("userid")
    {
        let auth = get_auth().await.read().await;
        if let Some(user) = auth.get_user_by_id(&userid)
            && !user.is_admin()
        {
            res.render(Redirect::other("/admin"));
            return Ok(());
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
pub async fn login(res: &mut Response, depot: &mut Depot, data: Login) -> AppResult<()> {
    let auth = get_auth().await.read().await;

    let user = auth.get_user_by_email_and_password(&data.email, &data.password);

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
pub async fn change_password(
    depot: &mut Depot,
    res: &mut Response,
    data: ChangePassword,
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
