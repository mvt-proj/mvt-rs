use salvo::prelude::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Health {
    title: String,
    message: String,
    timestamp: chrono::DateTime<chrono::Local>,
}

#[handler]
pub async fn get_health(res: &mut Response) {
    let data = Health {
        title: "MVT-RS".to_string(),
        message: "Simple and high-speed vector tile server developed in Rust".to_string(),
        timestamp: chrono::offset::Local::now(),
    };
    res.render(Json(&data));
}
