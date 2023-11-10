use salvo::basic_auth::BasicAuth;
use salvo::cache::{Cache, MokaStore, RequestIssuer};
use salvo::cors::{self as cors, Cors};
use salvo::http::Method;
use salvo::logging::Logger;
use salvo::prelude::*;
use salvo::serve_static::StaticDir;
use std::time::Duration;

use crate::{api, auth, health, html, tiles};


pub fn app_router() -> salvo::Router {
    let cache_30s = Cache::new(
        MokaStore::builder()
            .time_to_live(Duration::from_secs(30))
            .build(),
        RequestIssuer::default(),
    );

    let cors_handler = Cors::new()
        .allow_origin(cors::Any)
        .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(vec![
            "Content-Type",
            "Access-Control-Allow-Methods",
            "Access-Control-Allow-Headers",
            "Access-Control-Request-Method",
            "Access-Control-Allow-Origin",
            "Access-Control-Max-Age",
            "Authorization",
        ])
        .into_handler();

    let basic_auth_handler = BasicAuth::new(auth::Validator);
    let static_dir = StaticDir::new(["static"])
        .defaults("index.html")
        .listing(true);

    let router = Router::new()
        .hoop(Logger::default())
        .get(html::main::index)
        .push(Router::with_path("error404").get(html::main::error404))
        .push(Router::with_path("catalog").get(html::main::page_catalog))
        .push(Router::with_path("map/<layer_name>").get(html::main::page_map))
        .push(Router::with_path("health").get(health::get_health))
        .push(
            Router::with_path("admin")
                .hoop(basic_auth_handler)
                .get(html::admin::main::index)
                .push(
                    Router::with_path("users")
                        .get(html::admin::users::list_users)
                        .push(Router::with_path("new").get(html::admin::main::new_user))
                        .push(Router::with_path("create").post(html::admin::users::create_user))
                        .push(
                            Router::with_path("edit/<username>").get(html::admin::main::edit_user),
                        )
                        .push(Router::with_path("update").post(html::admin::users::update_user))
                        .push(
                            Router::with_path("delete/<username>")
                                .get(html::admin::users::delete_user),
                        ),
                )
                .push(
                    Router::with_path("catalog")
                        .get(html::admin::catalog::page_catalog)
                        .push(Router::with_path("layers/new").get(html::admin::main::new_layer))
                        .push(
                            Router::with_path("layers/create")
                                .post(html::admin::catalog::create_layer),
                        )
                        .push(
                            Router::with_path("layers/edit/<layer_name>")
                                .get(html::admin::main::edit_layer),
                        )
                        .push(
                            Router::with_path("layers/delete/<name>")
                                .get(html::admin::catalog::delete_layer),
                        )
                        .push(
                            Router::with_path("layers/update")
                                .post(html::admin::catalog::update_layer),
                        )
                        .push(
                            Router::with_path("layers/swichpublished/<layer_name>")
                                .get(html::admin::catalog::swich_published),
                        ),
                ),
        )
        .push(
            Router::with_path("api")
                .hoop(cors_handler.clone())
                .push(
                    Router::with_path("users/login")
                        .post(api::users::login)
                        .options(handler::empty())
                )
                .push(
                    Router::with_path("admin")
                        .hoop(auth::jwt_auth_handler())
                        .push(
                            Router::with_path("users").hoop(auth::validate_token).push(
                                Router::new()
                                    .get(api::users::index)
                                    .post(api::users::create),
                            ),
                        )
                        .push(
                            Router::with_path("database")
                                .hoop(auth::validate_token)
                                .push(Router::with_path("schemas")
                                      .get(api::database::schemas)
                                      )
                                .push(Router::with_path("tables/<schema>")
                                      .get(api::database::tables)
                                      )
                                .push(Router::with_path("fields/<schema>/<table>")
                                      .get(api::database::fields)
                                      )
                                .push(Router::with_path("srid/<schema>/<table>/<geometry>")
                                      .get(api::database::srid)
                                      )
                        )

                        .push(
                            Router::with_path("catalog/layer")
                                .hoop(auth::validate_token)
                                .get(api::catalog::list)
                                .post(api::catalog::create_layer)
                        )
                        .push(
                            Router::with_path("<**rest>")
                                .hoop(cors_handler.clone())
                                .options(handler::empty())
                        )
                ),
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
