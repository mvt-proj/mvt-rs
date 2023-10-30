use salvo::prelude::*;

use crate::{get_app_state, get_catalog, catalog::Layer};

#[handler]
pub async fn list(res: &mut Response) {
    let catalog = get_catalog().clone();
    res.render(Json(&catalog.layers));
}

#[handler]
// pub async fn create_layer(req: &mut Request, res: &mut Response) -> Result<Json<Layer>, StatusError>{
pub async fn create_layer(req: &mut Request, res: &mut Response) {
    // let catalog = get_catalog().clone();
    let app_state = get_app_state();
    let layer = req.parse_json::<Layer>().await;

    match layer {
        Ok(lyr) => {
            app_state.catalog.add_layer(lyr.clone()).await;
            res.render(Json(lyr))
        },
        Err(e) => res.render(format!("{:?}", e))
    }
}
