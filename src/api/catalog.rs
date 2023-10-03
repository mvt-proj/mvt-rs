use salvo::prelude::*;

use crate::get_catalog;

#[handler]
pub async fn prueba(res: &mut Response) {

    let catalog = get_catalog().clone();

    res.render(Json(&catalog.layers));
}
