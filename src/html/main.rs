use crate::VERSION;
use askama::Template;
use salvo::prelude::*;
use std::collections::{HashMap, HashSet};
use tokio::fs;

use crate::{
    auth::User,
    database::{query_extent, Extent},
    error::{AppError, AppResult},
    get_auth, get_catalog,
    models::{
        catalog::{Layer, StateLayer},
        styles::Style,
    },
};

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

    if is_auth {
        if let Some(session) = depot.session_mut() {
            if let Some(userid) = session.get::<String>("userid") {
                let auth = get_auth().await.read().await;
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
    version: String,
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
    translate: HashMap<String, String>,
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
    translate: HashMap<String, String>,
}

#[derive(Template)]
#[template(path = "sprites/index.html")]
struct SpritesTemplate {
    base: BaseTemplateData,
    sprites: Vec<String>,
}

#[derive(Template)]
#[template(path = "glyphs/index.html")]
struct GlyphsTemplate {
    base: BaseTemplateData,
    glyphs: Vec<String>,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    geometry: &'a str,
    layer: Layer,
    extent: Extent,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "mapview.html")]
struct MapViewTemplate {
    base: BaseTemplateData,
    style: Style,
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
    }
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let template = ChangePasswordTemplate { base };
    res.render(Text::Html(template.render().unwrap()));
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

    let layers: Vec<Layer> = if let Some(filter) = filter {
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

    let template = CatalogTableTemplate {
        layers: &layers,
        current_user: &user,
        is_guest_or_non_admin,
        translate,
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
    let layer_name = req.param::<String>("layer_name").unwrap();
    let parts: Vec<&str> = layer_name.split(':').collect();
    let category = parts.first().unwrap_or(&"").to_string();
    let name = parts.get(1).unwrap_or(&"").to_string(); // 🔹 Clonar para no mantener referencia

    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let (lyr, geometry) = {
        let catalog = get_catalog().await.read().await; // 🔓 Bloque limitado
        let lyr = catalog
            .find_layer_by_category_and_name(&category, &name, StateLayer::Published)
            .ok_or_else(|| {
                StatusError::not_found()
                    .brief("Layer not found")
                    .cause("The specified layer does not exist or is not published")
            })?
            .clone();

        let geometry = match lyr.geometry.as_str() {
            "points" => "circle".to_string(),
            "lines" => "line".to_string(),
            "polygons" => "fill".to_string(),
            _ => lyr.geometry.clone(),
        };

        (lyr, geometry)
    };

    let extent = query_extent(&lyr).await.unwrap();

    let template = MapTemplate {
        geometry: &geometry,
        layer: lyr,
        extent,
        base,
    };

    res.render(Text::Html(template.render().unwrap()));
    Ok(())
}

#[handler]
pub async fn page_map_view(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> Result<(), StatusError> {
    let style_id = req.param::<String>("style_id").unwrap();
    let style = Style::from_id(&style_id).await.unwrap();
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

    let template = MapViewTemplate { base, style };

    res.render(Text::Html(template.render().unwrap()));
    Ok(())
}

#[handler]
pub async fn page_styles(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };

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
    let (_is_auth, user) = get_session_data(depot).await;
    let mut styles = Style::get_all_styles().await?;

    if let Some(filter) = filter {
        styles.retain(|style| {
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
        });
    }
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();

    let template = StylesTableTemplate {
        styles: &styles,
        current_user: &user,
        translate,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn page_sprites(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };
    let dir_path = "map_assets/sprites";

    let entries = fs::read_dir(dir_path).await;

    if let Err(_err) = entries {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(AppError::NotFound(format!(
            "The directory {} does not exist",
            dir_path
        )));
    }

    let mut unique_names: HashSet<String> = HashSet::new();
    let mut entries = entries.unwrap();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir() {
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                unique_names.insert(dir_name.to_string());
            }
        }
    }

    let sprites: Vec<String> = unique_names.into_iter().collect();
    let template = SpritesTemplate { base, sprites };

    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn page_glyphs(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();
    let base = BaseTemplateData { is_auth, translate };
    let dir_path = "map_assets/glyphs";

    let entries = fs::read_dir(dir_path).await;

    if let Err(_err) = entries {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(AppError::NotFound(format!(
            "The directory {} does not exist",
            dir_path
        )));
    }

    let mut unique_names: HashSet<String> = HashSet::new();
    let mut entries = entries.unwrap();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir() {
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                unique_names.insert(dir_name.to_string());
            }
        }
    }
    let glyphs: Vec<String> = unique_names.into_iter().collect();
    let template = GlyphsTemplate { base, glyphs };
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
