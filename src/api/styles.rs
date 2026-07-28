use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    models::{category::Category, styles::Style},
};

#[handler]
pub async fn list(res: &mut Response) -> AppResult<()> {
    let styles = Style::get_all_styles().await?;
    res.render(Json(&styles));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewStyleRequest {
    name: String,
    category: String,
    description: String,
    style: String,
}

#[handler]
pub async fn create(res: &mut Response, data: NewStyleRequest) -> AppResult<()> {
    crate::services::utils::validate_style_json(&data.style)?;
    let category = Category::from_id(&data.category).await?;
    let style = Style::new(data.name, category, data.description, data.style).await?;
    res.render(Json(&style));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateStyleRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
    name: String,
    category: String,
    description: String,
    style: String,
}

#[handler]
pub async fn update(res: &mut Response, data: UpdateStyleRequest) -> AppResult<()> {
    let style = Style::from_id(&data.id).await?;
    crate::services::utils::validate_style_json(&data.style)?;
    let category = Category::from_id(&data.category).await?;
    let updated = style
        .update_style(data.name, category, data.description, data.style)
        .await?;
    res.render(Json(&updated));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    style.delete_style().await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
