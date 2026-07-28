use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::Group,
    error::{AppError, AppResult},
    get_auth,
};

#[handler]
pub async fn list(res: &mut Response) {
    let auth = get_auth().await.read().await;
    res.render(Json(&auth.groups));
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewGroupRequest {
    name: String,
    description: String,
}

#[handler]
pub async fn create(res: &mut Response, data: NewGroupRequest) -> AppResult<()> {
    let group = Group::new(data.name, data.description).await?;
    res.render(Json(&group));
    Ok(())
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct UpdateGroupRequest {
    #[salvo(extract(source(from = "param")))]
    id: String,
    name: String,
    description: String,
}

#[handler]
pub async fn update(res: &mut Response, data: UpdateGroupRequest) -> AppResult<()> {
    let group = Group::from_id(&data.id).await?;
    let updated = group.update_group(data.name, data.description).await?;
    res.render(Json(&updated));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let group = Group::from_id(&id).await?;
    group.delete_group().await?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}
