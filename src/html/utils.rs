use crate::auth::User;
use crate::get_auth;
use salvo::prelude::*;
use std::collections::HashMap;

pub struct BaseTemplateData {
    pub is_auth: bool,
    pub translate: HashMap<String, String>,
}

pub async fn is_authenticated(depot: &mut Depot) -> bool {
    if let Some(session) = depot.session_mut() {
        if session.is_expired() {
            return false;
        }
        if let Some(userid) = session.get::<String>("userid") {
            let auth = get_auth().await.read().await;
            if auth.get_user_by_id(&userid).is_some() {
                return true;
            }
        }
    }
    false
}

pub async fn get_session_data(depot: &mut Depot) -> (bool, Option<User>) {
    let is_auth = is_authenticated(depot).await;
    let mut user: Option<User> = None;

    if is_auth
        && let Some(session) = depot.session_mut()
        && let Some(userid) = session.get::<String>("userid")
    {
        let auth = get_auth().await.read().await;
        if let Some(usr) = auth.get_user_by_id(&userid) {
            user = Some(usr.clone());
        }
    }

    (is_auth, user)
}
