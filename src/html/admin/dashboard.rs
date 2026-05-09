use crate::error::AppResult;
use crate::html::utils::{BaseTemplateData, make_base};
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {
    base: BaseTemplateData,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let template = IndexTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}
