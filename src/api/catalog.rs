use salvo::prelude::*;

use crate::{models::catalog::Layer, get_app_state, get_catalog};

#[handler]
pub async fn list(req: &mut Request, res: &mut Response) {
    let catalog = get_catalog().clone();
    let mut layers = catalog.layers;
    dbg!(&req);
    let scheme = req.scheme().to_string();

    let host = req.headers().get("host").unwrap().to_str().unwrap();

    for layer in &mut layers {
        layer.url = Some(format!(
            "{scheme}://{host}/tiles/{}/{{z}}/{{x}}]/{{y}}].pbf",
            layer.name
        ));
    }

    res.render(Json(&layers));
}

#[handler]
// pub async fn create_layer(req: &mut Request, res: &mut Response) -> Result<Json<Layer>, StatusError>{
pub async fn create_layer(req: &mut Request, res: &mut Response) {
    // let catalog = get_catalog().clone();
    let app_state = get_app_state();
    let layer = req.parse_json::<Layer>().await;

    match layer {
        Ok(lyr) => {
            let _ = app_state.catalog.add_layer(lyr.clone()).await;
            res.render(Json(lyr))
        }
        Err(e) => res.render(format!("{:?}", e)),
    }
}
