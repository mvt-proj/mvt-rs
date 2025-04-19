use crate::VERSION;
use salvo::prelude::*;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Serialize)]
pub struct Health {
    title: String,
    message: String,
    #[serde(with = "time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    version: String,
}

#[handler]
pub async fn get_health(res: &mut Response) {
    let data = Health {
        title: "MVT Server".to_string(),
        message: "Simple and high-speed vector tiles server developed in Rust".to_string(),
        timestamp: OffsetDateTime::now_utc(),
        version: VERSION.to_string(),
    };
    res.render(Json(&data));
}
