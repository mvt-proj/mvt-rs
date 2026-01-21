use super::utils::{BaseTemplateData, get_session_data, is_authenticated};
use crate::auth::User;
use crate::error::AppResult;
use crate::get_catalog;
use crate::models::catalog::Layer;
use askama::Template;
use salvo::prelude::*;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "catalog/catalog.html")]
struct CatalogTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "catalog/table.html")]
struct CatalogTableTemplate<'a> {
    layers: &'a Vec<Layer>,
    current_user: &'a Option<User>,
    is_guest_or_non_admin: bool,
    translate: HashMap<String, String>,
}

#[handler]
pub async fn page_catalog(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let template = CatalogTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn table_catalog(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");
    let catalog = get_catalog().await.read().await;
    let (_is_auth, user) = get_session_data(depot).await;

    let mut layers: Vec<Layer> = if let Some(filter) = filter {
        catalog
            .layers
            .iter()
            .filter(|layer| {
                layer.alias.to_lowercase().contains(&filter.to_lowercase())
                    || layer
                        .category
                        .name
                        .to_lowercase()
                        .contains(&filter.to_lowercase())
                    || layer.name.to_lowercase().contains(&filter.to_lowercase())
            })
            .cloned()
            .collect()
    } else {
        catalog.layers.clone()
    };

    let is_guest_or_non_admin = user.is_none() || user.as_ref().is_none_or(|usr| !usr.is_admin());
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();

    Layer::sort_by_category_and_name(&mut layers);
    let template = CatalogTableTemplate {
        layers: &layers,
        current_user: &user,
        is_guest_or_non_admin,
        translate,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
