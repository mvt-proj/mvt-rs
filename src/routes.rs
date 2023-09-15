use std::time::Duration;

use salvo::affix;
use salvo::cache::{Cache, MokaStore, RequestIssuer};
use salvo::cors::{self as cors, Cors};
use salvo::http::Method;
use salvo::logging::Logger;
use salvo::prelude::*;

use crate::{html::{index, mapview}, tiles, health, Config};

pub fn app_router(config: Config) -> salvo::Router {
    let cache_30s = Cache::new(
        MokaStore::builder()
            .time_to_live(Duration::from_secs(30))
            .build(),
        RequestIssuer::default(),
    );

    let cors_handler = Cors::new()
        .allow_origin(cors::Any)
        .allow_methods(vec![Method::GET, Method::OPTIONS])
        .allow_headers(vec![
            "CONTENT-TYPE",
            "content-type",
            "Access-Control-Request-Method",
            "Access-Control-Allow-Origin",
            "Access-Control-Allow-Headers",
            "Access-Control-Max-Age",
            "authorization",
            "Authorization",
        ])
        .into_handler();

    let router = Router::new()
        .hoop(Logger::default())
        .hoop(affix::inject(config.clone()))
        .get(index)
        .push(Router::with_path("/map/<layer>").get(mapview))
        .push(Router::with_path("/health").get(health::get_health))
        .push(Router::with_path("/tiles").get(tiles::mvt))
        .push(
            Router::with_path("/tiles/<layer>/<z>/<x>/<y>.pbf")
                .hoop(cache_30s)
                .hoop(cors_handler)
                .options(handler::empty())
                .get(tiles::mvt),
        );
    router
}
