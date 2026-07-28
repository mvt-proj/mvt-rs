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
            res.status_code(StatusCode::UNAUTHORIZED);
            res.render(Json(&state));
        }
        JwtAuthState::Forbidden => {
            let state = AuthorizeState {
                message: "Forbidden".to_string(),
                status_code: 403,
            };
            res.status_code(StatusCode::FORBIDDEN);
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

#[handler]
pub async fn require_api_admin(depot: &mut Depot) -> AppResult<()> {
    let is_admin = depot
        .jwt_auth_data::<JwtClaims>()
        .is_some_and(|data| data.claims.is_admin());

    if is_admin {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin privileges required".to_string()))
    }
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

    let user = auth.get_user_by_email_and_password(&data.email, &data.password)?;

    let mut session = Session::new();
    session
        .insert("userid", user.id.clone())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    depot.set_session(session);

    res.render(Redirect::other("/"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use salvo::test::TestClient;
    use time::{Duration, OffsetDateTime};

    fn ensure_jwt_secret() {
        // `jsonwebtoken` 10.4 requires a process-level `CryptoProvider` before any
        // encode/decode call. Cargo feature unification pulls in both `rust_crypto`
        // (this crate's declared choice) and `aws_lc_rs` (via salvo's default
        // `jwt-auth` feature), so the crate can't auto-select one and panics unless
        // we install a provider explicitly. This mirrors what production code needs
        // too (see concern in task report) but is scoped here to keep this task's
        // diff minimal.
        let _ = jsonwebtoken::crypto::CryptoProvider::install_default(
            &jsonwebtoken::crypto::rust_crypto::DEFAULT_PROVIDER,
        );
        let _ = crate::JWT_SECRET.set("test-only-secret-not-used-in-prod".to_string());
    }

    fn sign_token(groups: Vec<String>) -> String {
        ensure_jwt_secret();
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "tester".to_string(),
            email: "tester@test.com".to_string(),
            groups,
            exp: (OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(crate::get_jwt_secret().as_bytes()),
        )
        .unwrap()
    }

    fn protected_router() -> Router {
        #[handler]
        async fn ok(res: &mut Response) {
            res.render("ok");
        }

        Router::new()
            .hoop(jwt_auth_handler())
            .hoop(require_api_admin)
            .get(ok)
    }

    #[tokio::test]
    async fn require_api_admin_allows_admin_token() {
        let token = sign_token(vec!["admin".to_string()]);
        let service = Service::new(protected_router());
        let res = TestClient::get("http://127.0.0.1:5800/")
            .bearer_auth(token)
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_api_admin_rejects_non_admin_token() {
        let token = sign_token(vec!["users".to_string()]);
        let service = Service::new(protected_router());
        let res = TestClient::get("http://127.0.0.1:5800/")
            .bearer_auth(token)
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn require_api_admin_rejects_missing_token() {
        ensure_jwt_secret();
        let service = Service::new(protected_router());
        let res = TestClient::get("http://127.0.0.1:5800/").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }
}
