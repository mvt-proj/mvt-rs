use askama::Template;
use salvo::prelude::*;

use crate::{
    auth::{Auth, User},
    error::AppResult,
    get_auth, get_catalog,
    models::{
        catalog::{Catalog, Layer, StateLayer},
        styles::Style,
    },
};

pub struct BaseTemplateData {
    pub is_auth: bool,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    base: BaseTemplateData,
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
#[template(path = "error404.html")]
struct E404Template {
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
struct StylesTemplate<'a> {
    styles: &'a Vec<Style>,
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
    category: &'a str,
    name: &'a str,
    alias: &'a str,
    geometry: &'a str,
    base: BaseTemplateData,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) {
    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

    let base = BaseTemplateData { is_auth };

    let template = IndexTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn login(res: &mut Response, depot: &mut Depot) {
    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

    let base = BaseTemplateData { is_auth };

    let template = LoginTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn change_password(res: &mut Response, depot: &mut Depot) {
    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

    let base = BaseTemplateData { is_auth };

    let template = ChangePasswordTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn error404(res: &mut Response, depot: &mut Depot) {
    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

    let base = BaseTemplateData { is_auth };

    let template = E404Template { base };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn page_catalog(res: &mut Response, depot: &mut Depot) {
    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

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
    let mut user: Option<User> = None;

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

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(usr) = auth.get_user_by_id(&userid) {
                user = Some(usr.clone());
            }
        }
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

    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

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

    let template = MapTemplate {
        category: &lyr.category.name,
        name: &lyr.name,
        alias: &lyr.alias,
        geometry,
        base,
    };

    res.render(Text::Html(template.render().unwrap()));
    Ok(())
}

#[handler]
pub async fn page_styles(res: &mut Response, depot: &mut Depot) {
    let styles = Style::get_all_styles().await.unwrap();
    let mut is_auth = false;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(_) = auth.get_user_by_id(&userid) {
                is_auth = true
            }
        }
    }

    let base = BaseTemplateData { is_auth };

    let template = StylesTemplate {
        styles: &styles,
        base,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn table_styles(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");
    let mut user: Option<User> = None;

    if let Some(session) = depot.session_mut() {
        if let Some(userid) = session.get::<String>("userid") {
            let auth: Auth = get_auth().clone();
            if let Some(usr) = auth.get_user_by_id(&userid) {
                user = Some(usr.clone());
            }
        }
    }

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
