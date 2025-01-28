use askama::Template;
use salvo::prelude::*;

use crate::{
    auth::{Auth, User},
    error::AppResult,
    get_auth, get_catalog,
    database::{Extent, query_extent},
    models::{
        catalog::{Catalog, Layer, StateLayer},
        styles::Style
    },
};

pub struct BaseTemplateData {
    pub is_auth: bool,
}

pub fn is_authenticated(depot: &mut Depot) -> bool {
    if let Some(session) = depot.session_mut() {
        if session.is_expired() {
            return false;
        }
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if auth.get_user_by_id(&userid).is_some() {
                return true;
            }
        }
    }
    false
}

pub fn get_session_data(depot: &mut Depot) -> (bool, Option<User>) {
    let is_auth = is_authenticated(depot);
    let mut user: Option<User> = None;

    if is_auth {
        if let Some(session) = depot.session_mut() {
            if let Some(userid) = session.get::<String>("userid") {
                let auth: Auth = get_auth().clone();
                if let Some(usr) = auth.get_user_by_id(&userid) {
                    user = Some(usr.clone());
                }
            }
        }
    }
    (is_auth, user)
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub status: u16,
    pub message: String,
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
}

#[derive(Template)]
#[template(path = "styles/styles.html")]
struct StylesTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "styles/table.html")]
struct StylesTableTemplate<'a> {
    styles: &'a Vec<Style>,
    current_user: &'a Option<User>,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    geometry: &'a str,
    layer: Layer,
    extent: Extent,
    base: BaseTemplateData,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot);
    let base = BaseTemplateData { is_auth };

    let template = IndexTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn login(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot);
    if is_auth {
        res.render(Redirect::other("/"));
    }

    let base = BaseTemplateData { is_auth };

    let template = LoginTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn change_password(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot);

    if !is_auth {
        res.render(Redirect::other("/login"));
    }
    let base = BaseTemplateData { is_auth };

    let template = ChangePasswordTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn page_catalog(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot);

    let base = BaseTemplateData { is_auth };

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
    let mut catalog: Catalog = get_catalog().clone();
    let (_is_auth, user) = get_session_data(depot);

    if let Some(filter) = filter {
        catalog.layers = catalog
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
            .collect();
    }

    let is_guest_or_non_admin = user.is_none() || user.as_ref().map_or(true, |usr| !usr.is_admin());

    let template = CatalogTableTemplate {
        layers: &catalog.layers,
        current_user: &user,
        is_guest_or_non_admin,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn page_map(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> Result<(), StatusError> {
    let catalog: Catalog = get_catalog().clone();
    let layer_name = req.param::<String>("layer_name").unwrap();
    let parts: Vec<&str> = layer_name.split(':').collect();
    let category = parts.first().unwrap_or(&"");
    let name = parts.get(1).unwrap_or(&"");

    let is_auth = is_authenticated(depot);

    let base = BaseTemplateData { is_auth };

    let lyr = catalog
        .find_layer_by_category_and_name(category, name, StateLayer::Published)
        .ok_or_else(|| {
            StatusError::not_found()
                .brief("Layer not found")
                .cause("The specified layer does not exist or is not published")
        })?;

    let geometry = match lyr.geometry.as_str() {
        "points" => "circle",
        "lines" => "line",
        "polygons" => "fill",
        _ => &lyr.geometry,
    };

    let extent = query_extent(&lyr).await.unwrap();

    let template = MapTemplate {
        geometry,
        layer: lyr.clone(),
        extent,
        base,
    };

    res.render(Text::Html(template.render().unwrap()));
    Ok(())
}

#[handler]
pub async fn page_styles(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot);
    let base = BaseTemplateData { is_auth };

    let template = StylesTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn table_styles(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");
    let (_is_auth, user) = get_session_data(depot);
    let mut styles = Style::get_all_styles().await?;

    if let Some(filter) = filter {
        styles = styles
            .into_iter()
            .filter(|style| {
                style.name.to_lowercase().contains(&filter.to_lowercase())
                    || style
                        .description
                        .to_lowercase()
                        .contains(&filter.to_lowercase())
                    || style
                        .category
                        .name
                        .to_lowercase()
                        .contains(&filter.to_lowercase())
            })
            .collect();
    }

    let template = StylesTableTemplate {
        styles: &styles,
        current_user: &user,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn handle_errors(res: &mut Response, ctrl: &mut FlowCtrl) -> AppResult<()> {
    if let Some(status) = res.status_code {
        if status.as_u16() >= 400 && status.as_u16() <= 600 {
            let template = ErrorTemplate {
                status: status.as_u16(),
                message: status.canonical_reason().unwrap().to_string(),
            };

            res.render(Text::Html(template.render()?));
            ctrl.skip_rest();
            return Ok(());
        }
    }

    Ok(())
}
