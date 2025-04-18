use crate::{error::AppResult, models::styles::Style};
use salvo::prelude::*;
use serde_json;

#[handler]
pub async fn index(req: &mut Request, _res: &mut Response) -> AppResult<Json<serde_json::Value>> {
    let style_name = req.param::<String>("style_name").unwrap_or("".to_string());
    let parts: Vec<&str> = style_name.split(':').collect();

    let category = parts.first().unwrap_or(&"");
    let name = parts.get(1).unwrap_or(&"");
    let style = Style::from_category_and_name(category, name).await?;

    Ok(Json(serde_json::from_str(style.style.as_str()).unwrap()))
}
