use super::utils::{BaseTemplateData, make_base};
use crate::error::AppResult;
use crate::models::styles::Style;
use askama::Template;
use salvo::prelude::*;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "styles/styles.html")]
struct StylesTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "styles/table.html")]
struct StylesTableTemplate<'a> {
    styles: &'a Vec<Style>,
    is_admin_context: bool,
    translate: HashMap<String, String>,
}

#[handler]
pub async fn page_styles(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;

    // Admins have their own styles page with management actions; keep them
    // off the read-only public one so they don't land on it by habit.
    if user.is_some_and(|u| u.is_admin()) {
        res.render(Redirect::other("/admin/styles"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    }

    let template = StylesTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

/// Renders the styles table fragment shared by the public `/styles` page and
/// the `/admin/styles` page. `is_admin_context` must reflect which route
/// served the request, not the viewer's role: the public page has no
/// `openModal`/management JS loaded, so it must stay read-only even for an
/// admin who lands on it directly.
async fn render_styles_table(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    is_admin_context: bool,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");
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

    Style::sort_by_category_and_name(&mut styles);
    let template = StylesTableTemplate {
        styles: &styles,
        is_admin_context,
        translate,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn table_styles(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    render_styles_table(req, res, depot, false).await
}

#[handler]
pub async fn table_styles_admin(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    render_styles_table(req, res, depot, true).await
}
