use super::utils::{BaseTemplateData, make_base};
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
    is_admin_context: bool,
    translate: HashMap<String, String>,
}

#[handler]
pub async fn page_catalog(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;

    // Admins have their own catalog page with management actions; keep them
    // off the read-only public one so they don't land on it by habit.
    if user.is_some_and(|u| u.is_admin()) {
        res.render(Redirect::other("/admin/catalog"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    }

    let template = CatalogTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

/// Renders the catalog table fragment shared by the public `/catalog` page and
/// the `/admin/catalog` page. `is_admin_context` must reflect which route
/// served the request, not the viewer's role: the public page has no
/// `openModal`/management JS loaded, so it must stay read-only even for an
/// admin who lands on it directly.
async fn render_catalog_table(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    is_admin_context: bool,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");
    let catalog = get_catalog().await.read().await;

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

    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();

    Layer::sort_by_category_and_name(&mut layers);
    let template = CatalogTableTemplate {
        layers: &layers,
        is_admin_context,
        translate,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn table_catalog(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    render_catalog_table(req, res, depot, false).await
}

#[handler]
pub async fn table_catalog_admin(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    render_catalog_table(req, res, depot, true).await
}
