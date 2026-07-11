use salvo::prelude::*;

use crate::{
    error::{AppError, AppResult},
    get_catalog,
    models::catalog::Layer,
};

#[handler]
pub async fn list(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let catalog = get_catalog().await.read().await;
    let mut layers = catalog.layers.clone();
    let scheme = req.scheme().to_string();

    let host = req
        .headers()
        .get("host")
        .ok_or(AppError::RequestParamError("Missing host header".to_string()))?
        .to_str()
        .map_err(|_| AppError::RequestParamError("Invalid host header encoding".to_string()))?;

    for layer in &mut layers {
        layer.url = Some(format!(
            "{scheme}://{host}/services/tiles/{}:{}/{{z}}/{{x}}]/{{y}}].pbf",
            layer.category.name, layer.name
        ));
    }

    res.render(Json(&layers));
    Ok(())
}

#[handler]
pub async fn create_layer(req: &mut Request, res: &mut Response) -> AppResult<()> {
    match req.parse_json::<Layer>().await {
        Ok(mut lyr) => {
            lyr.name = crate::services::utils::normalize_name(&lyr.name)?;
            let mut catalog = get_catalog().await.write().await;
            catalog.add_layer(lyr.clone()).await?;
            res.render(Json(lyr));
        }
        Err(e) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(format!("{e:?}"));
        }
    }
    Ok(())
}
