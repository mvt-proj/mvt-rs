use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    get_categories,
    models::category::Category,
};

#[handler]
pub async fn list(res: &mut Response) {
    let categories = get_categories().await.read().await;
    res.render(Json(categories.to_vec()));
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewCategoryRequest {
    name: String,
    description: String,
}

#[handler]
pub async fn create(res: &mut Response, data: NewCategoryRequest) -> AppResult<()> {
    let category = Category::new(data.name, data.description).await?;
    res.render(Json(&category));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateCategoryRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
    name: String,
    description: String,
}

#[handler]
pub async fn update(res: &mut Response, data: UpdateCategoryRequest) -> AppResult<()> {
    let category = Category::from_id(&data.id).await?;
    let updated = category.update_category(data.name, data.description).await?;
    res.render(Json(&updated));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let category = Category::from_id(&id).await?;
    category.delete_category().await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
