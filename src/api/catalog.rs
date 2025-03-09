use salvo::prelude::*;

use crate::{get_catalog, models::catalog::Layer};

#[handler]
pub async fn list(req: &mut Request, res: &mut Response) {
    let catalog = get_catalog().await.read().await;
    let mut layers = catalog.layers.clone();
    let scheme = req.scheme().to_string();

    let host = req.headers().get("host").unwrap().to_str().unwrap();

    for layer in &mut layers {
        layer.url = Some(format!(
            "{scheme}://{host}/services/tiles/{}:{}/{{z}}/{{x}}]/{{y}}].pbf",
            layer.category.name, layer.name
        ));
    }

    res.render(Json(&layers));
}

#[handler]
pub async fn create_layer(req: &mut Request, res: &mut Response) {
    let layer = req.parse_json::<Layer>().await;

    match layer {
        Ok(lyr) => {
            let mut catalog = get_catalog().await.write().await;
            let _ = catalog.add_layer(lyr.clone()).await;
            res.render(Json(lyr))
        }
        Err(e) => res.render(format!("{:?}", e)),
    }
}
