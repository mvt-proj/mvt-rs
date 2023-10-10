use askama::Template;
use salvo::prelude::*;
use crate::{
    catalog::{Layer, StateLayer},
    get_catalog
};

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {}

#[derive(Template)]
#[template(path = "admin/newuser.html")]
struct NewUserTemplate {}

#[derive(Template)]
#[template(path = "admin/newlayer.html")]
struct NewLayerTemplate {}

#[derive(Template)]
#[template(path = "admin/editlayer.html")]
struct EditLayerTemplate {
    layer: Layer
}

#[handler]
pub async fn index(res: &mut Response) {
    let template = IndexTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn new_user(res: &mut Response) {
    let template = NewUserTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn new_layer(res: &mut Response) {
    let template = NewLayerTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn edit_layer(req: &mut Request, res: &mut Response) {
    let layer_name = req.param::<String>("layer_name").unwrap();
    let catalog = get_catalog().clone();
    let layer = catalog.find_layer_by_name(&layer_name, StateLayer::ANY).unwrap();
    let template = EditLayerTemplate {
        layer: layer.clone()
    };
    res.render(Text::Html(template.render().unwrap()));
}
