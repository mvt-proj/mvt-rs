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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::category::Category;

    /// Every `translate[...]` key `catalog/table.html` looks up. Missing a
    /// key here panics at render time (`HashMap::index` on absent key), so
    /// this must track the template's real usages, not just the ones this
    /// test cares about.
    fn full_translate() -> HashMap<String, String> {
        [
            "category",
            "database",
            "layer-name",
            "alias",
            "table",
            "copy",
            "inspect-layer",
            "info",
            "switch-published",
            "confirm-delete-cache",
            "delete-cache",
            "edit",
            "confirm-delete-layer",
            "delete",
        ]
        .into_iter()
        .map(|k| (k.to_string(), k.to_string()))
        .collect()
    }

    fn test_layer(id: &str, published: bool) -> Layer {
        Layer {
            id: id.to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "public".to_string(),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "parcels".to_string(),
            alias: "Parcels".to_string(),
            description: "Cadastral parcels".to_string(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "parcels".to_string(),
            fields: vec!["gid".to_string()],
            filter: None,
            srid: None,
            geom: None,
            label_layer: false,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age: None,
            max_records: None,
            published,
            url: None,
            groups: None,
        }
    }

    /// Spec "Metadata entry in catalog row actions": the admin dropdown
    /// shows a "Metadatos" entry only for rows whose layer is `published`.
    #[test]
    fn admin_dropdown_shows_metadata_entry_only_for_published_layer() {
        let layers = vec![test_layer("layer-published", true), test_layer("layer-unpublished", false)];
        let template = CatalogTableTemplate {
            layers: &layers,
            is_admin_context: true,
            translate: full_translate(),
        };
        let html = template.render().unwrap();

        assert_eq!(
            html.matches(">Metadatos<").count(),
            1,
            "must show exactly one rendered Metadatos entry"
        );
        assert!(
            html.contains("/admin/metadata/edit/layer-published"),
            "must link to the published layer's metadata form"
        );
        assert!(
            !html.contains("/admin/metadata/edit/layer-unpublished"),
            "must not link to the unpublished layer's metadata form"
        );
    }

    /// The public (non-admin) catalog view never shows management actions,
    /// including this one — it is scoped to the admin dropdown only.
    #[test]
    fn public_context_never_shows_metadata_entry() {
        let layers = vec![test_layer("layer-published", true)];
        let template = CatalogTableTemplate {
            layers: &layers,
            is_admin_context: false,
            translate: full_translate(),
        };
        let html = template.render().unwrap();

        assert!(!html.contains("Metadatos"));
    }
}
