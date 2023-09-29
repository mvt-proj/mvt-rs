use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "admin/index.html")]
struct IndexTemplate {}

#[handler]
pub async fn index(res: &mut Response) {
    let template = IndexTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}

#[derive(Template)]
#[template(path = "admin/newuser.html")]
struct NewUserTemplate {}

#[handler]
pub async fn new_user(res: &mut Response) {
    let template = NewUserTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}

#[derive(Template)]
#[template(path = "admin/newlayer.html")]
struct NewLayerTemplate {}

#[handler]
pub async fn new_layer(res: &mut Response) {
    let template = NewLayerTemplate {};
    res.render(Text::Html(template.render().unwrap()));
}
