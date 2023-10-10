use salvo::basic_auth::BasicAuth;
use salvo::cache::{Cache, MokaStore, RequestIssuer};
use salvo::cors::{self as cors, Cors};
use salvo::http::Method;
use salvo::logging::Logger;
use salvo::prelude::*;
use salvo::serve_static::StaticDir;
use std::time::Duration;

use crate::{auth, health, api, html, tiles};

pub fn app_router() -> salvo::Router {
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

    let auth_handler = BasicAuth::new(auth::Validator);
    let static_dir = StaticDir::new(["static"])
        .defaults("index.html")
        .listing(true);

    let router = Router::new()
        .hoop(Logger::default())
        // .hoop(affix::inject(config.clone()))
        .get(html::main::index)
        .push(Router::with_path("catalog").get(html::main::page_catalog))
        .push(Router::with_path("map/<layer_name>").get(html::main::page_map))
        .push(Router::with_path("health").get(health::get_health))
        .push(
            Router::with_path("admin")
                .hoop(auth_handler)
                .get(html::admin::main::index)
                .push(Router::with_path("catalog").get(html::admin::catalog::page_catalog))
                .push(Router::with_path("users").get(html::admin::users::list_users))
                .push(Router::with_path("newuser").get(html::admin::main::new_user))
                .push(Router::with_path("createuser").post(html::admin::users::create_user))
                .push(Router::with_path("deleteuser/<username>").get(html::admin::users::delete_user))
                .push(Router::with_path("newlayer").get(html::admin::main::new_layer))
                .push(Router::with_path("editlayer/<layer_name>").get(html::admin::main::edit_layer))
                .push(Router::with_path("createlayer").post(html::admin::catalog::create_layer))
                .push(Router::with_path("updatelayer").post(html::admin::catalog::update_layer))
                .push(Router::with_path("swichpublished/<layer_name>").get(html::admin::catalog::swich_published))
        )
        .push(
            Router::with_path("api")
                // .(html::admin::main::index)
                // .hoop(auth_handler)
                .push(Router::with_path("users/login").post(api::users::login))
                // .push(Router::with_path("/users").get(html::admin::users::list_users))
                .push(
                    Router::with_path("admin")
                        .hoop(auth::jwt_auth_handler())
                        // .get(html::admin::main::index)
                        .push(
                            Router::with_path("users").hoop(auth::validate_token)
                            .push(Router::new()
                                .get(api::users::index)
                                .post(api::users::create))
                        )
                        .push(Router::with_path("catalog").hoop(auth::validate_token).get(api::catalog::prueba))
                )

        )
        .push(Router::with_path("tiles").get(tiles::mvt))
        .push(
            Router::with_path("tiles/<layer_name>/<z>/<x>/<y>.pbf")
                .hoop(cache_30s)
                .hoop(cors_handler)
                .options(handler::empty())
                .get(tiles::mvt),
        )
        .push(Router::with_path("static/<**path>").get(static_dir));
    router
}
