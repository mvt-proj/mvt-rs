use crate::html::utils::{BaseTemplateData, is_authenticated};
use askama::Template;
use salvo::prelude::*;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {
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

    let template = IndexTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}
