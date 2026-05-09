use super::utils::{BaseTemplateData, make_base};
use crate::auth::User;
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
    current_user: &'a Option<User>,
    translate: HashMap<String, String>,
}

#[handler]
pub async fn page_styles(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let template = StylesTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn table_styles(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
) -> AppResult<()> {
    let filter = req.query::<String>("filter");
    let (_, user) = make_base(depot).await;
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
        current_user: &user,
        translate,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}
