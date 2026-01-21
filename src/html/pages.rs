use super::utils::{BaseTemplateData, is_authenticated};
use crate::VERSION;
use askama::Template;
use salvo::prelude::*;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    base: BaseTemplateData,
    version: String,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "changepassword.html")]
struct ChangePasswordTemplate {
    base: BaseTemplateData,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();

    let base = BaseTemplateData { is_auth, translate };
    let template = IndexTemplate {
        base,
        version: VERSION.to_string(),
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn login(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot).await;
    if is_auth {
        res.render(Redirect::other("/"));
        return;
    }

    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let template = LoginTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn change_password(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot).await;
    if !is_auth {
        res.render(Redirect::other("/login"));
        return;
    }

    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let template = ChangePasswordTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}
