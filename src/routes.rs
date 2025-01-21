use salvo::cache::{Cache, MokaStore, RequestIssuer};
use salvo::catcher::Catcher;
use salvo::cors::{self as cors, Cors};
// use salvo::http::Method;
use salvo::logging::Logger;
use salvo::prelude::*;
use salvo::serve_static::StaticDir;
use salvo::session::CookieStore;
use std::time::Duration;

use crate::{
    api, auth, health, html,
    services::{styles, tiles},
};

pub fn app_router() -> Service {
    let cache_30s = Cache::new(
        MokaStore::builder()
            .time_to_live(Duration::from_secs(30))
            .build(),
        RequestIssuer::default(),
    );

    let cors_handler = Cors::new()
        .allow_origin(cors::Any)
        .allow_methods(cors::Any)
        .allow_headers(cors::Any)
        .into_handler();

    let session_handler = SessionHandler::builder(
        CookieStore::new(),
        b"a2b59cdf777c0d2802e825617849f355b82c0926212bb6302abc239d8f67ba87",
    )
    .build()
    .unwrap();

    let static_dir = StaticDir::new(["static"])
        .defaults("index.html")
        .auto_list(true);

    let router = Router::new()
        .hoop(Logger::default())
        .hoop(session_handler)
        .get(html::main::index)
        .push(Router::with_path("login").get(html::main::login))
        .push(
            Router::with_path("logout")
                .hoop(auth::session_auth_handler)
                .get(auth::logout),
        )
        .push(Router::with_path("auth/login").post(auth::login))
        .push(
            Router::with_path("changepassword")
                .hoop(auth::session_auth_handler)
                .get(html::main::change_password),
        )
        .push(
            Router::with_path("auth/changepassword")
                .hoop(auth::session_auth_handler)
                .post(auth::change_password),
        )
        .push(Router::with_path("catalog").get(html::main::page_catalog))
        .push(Router::with_path("catalogtable").get(html::main::table_catalog))
        .push(Router::with_path("styles").get(html::main::page_styles))
        .push(Router::with_path("styletable").get(html::main::table_styles))
        .push(Router::with_path("map/{layer_name}").get(html::main::page_map))
        .push(Router::with_path("health").get(health::get_health))
        .push(
            Router::with_path("admin")
                .hoop(auth::session_auth_handler)
                .get(html::admin::main::index)
                .push(
                    Router::with_path("users")
                        .hoop(auth::require_user_admin)
                        .get(html::admin::users::list_users)
                        .push(Router::with_path("new").get(html::admin::main::new_user))
                        .push(Router::with_path("create").post(html::admin::users::create_user))
                        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_user))
                        .push(Router::with_path("update").post(html::admin::users::update_user))
                        .push(
                            Router::with_path("delete/{id}").get(html::admin::users::delete_user),
                        ),
                )
                .push(
                    Router::with_path("categories")
                        .hoop(auth::require_user_admin)
                        .get(html::admin::categories::list_categories)
                        .push(Router::with_path("new").get(html::admin::main::new_category))
                        .push(
                            Router::with_path("create")
                                .post(html::admin::categories::create_category),
                        )
                        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_category))
                        .push(
                            Router::with_path("update")
                                .post(html::admin::categories::edit_category),
                        )
                        .push(
                            Router::with_path("delete/{id}")
                                .get(html::admin::categories::delete_category),
                        ),
                )
                .push(
                    Router::with_path("styles")
                        .hoop(auth::require_user_admin)
                        .get(html::admin::styles::list_styles)
                        .push(Router::with_path("new").get(html::admin::main::new_style))
                        .push(Router::with_path("create").post(html::admin::styles::create_style))
                        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_style))
                        .push(Router::with_path("update").post(html::admin::styles::edit_style))
                        .push(
                            Router::with_path("delete/{id}").get(html::admin::styles::delete_style),
                        ),
                )
                .push(
                    Router::with_path("groups")
                        .hoop(auth::require_user_admin)
                        .get(html::admin::groups::list_groups)
                        .push(Router::with_path("new").get(html::admin::main::new_group))
                        .push(Router::with_path("create").post(html::admin::groups::create_group))
                        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_group))
                        .push(Router::with_path("update").post(html::admin::groups::edit_group))
                        .push(
                            Router::with_path("delete/{id}").get(html::admin::groups::delete_group),
                        ),
                )
                .push(
                    Router::with_path("catalog")
                        .hoop(auth::require_user_admin)
                        .get(html::admin::catalog::page_catalog)
                        .push(Router::with_path("layers/new").get(html::admin::main::new_layer))
                        .push(
                            Router::with_path("layers/create")
                                .post(html::admin::catalog::create_layer),
                        )
                        .push(
                            Router::with_path("layers/edit/{id}")
                                .get(html::admin::main::edit_layer),
                        )
                        .push(
                            Router::with_path("layers/delete/{id}")
                                .get(html::admin::catalog::delete_layer),
                        )
                        .push(
                            Router::with_path("layers/update")
                                .post(html::admin::catalog::update_layer),
                        )
                        .push(
                            Router::with_path("layers/swichpublished/{id}")
                                .get(html::admin::catalog::swich_published),
                        )
                        .push(
                            Router::with_path("layers/delete_cache/{id}")
                                .get(html::admin::catalog::delete_layer_cache),
                        ),
                )
                .push(
                    Router::with_path("database")
                        .push(Router::with_path("schemas").get(html::admin::database::schemas))
                        .push(Router::with_path("tables").get(html::admin::database::tables))
                        .push(Router::with_path("fields").get(html::admin::database::fields))
                        .push(Router::with_path("srid").get(html::admin::database::srid)),
                ),
        )
        .push(
            Router::with_path("api")
                .hoop(cors_handler.clone())
                .push(
                    Router::with_path("users/login")
                        .post(api::users::login)
                        .options(handler::empty()),
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
                                .push(Router::with_path("schemas").get(api::database::schemas))
                                .push(
                                    Router::with_path("tables/{schema}").get(api::database::tables),
                                )
                                .push(
                                    Router::with_path("fields/{schema}/{table}")
                                        .get(api::database::fields),
                                )
                                .push(
                                    Router::with_path("srid/{schema}/{table}/{geometry}")
                                        .get(api::database::srid),
                                ),
                        )
                        .push(
                            Router::with_path("catalog/layer")
                                .hoop(auth::validate_token)
                                .get(api::catalog::list)
                                .post(api::catalog::create_layer),
                        )
                        .push(
                            Router::with_path("{**rest}")
                                .hoop(cors_handler.clone())
                                .options(handler::empty()),
                        ),
                ),
        )
        .push(
            Router::with_path("services")
                .hoop(cache_30s)
                .hoop(cors_handler)
                .options(handler::empty())
                .push(Router::with_path("tiles").get(tiles::mvt))
                .push(Router::with_path("tiles/{layer_name}/{z}/{x}/{y}.pbf").get(tiles::mvt))
                .push(Router::with_path("styles/{style_name}").get(styles::index)),
        )
        .push(Router::with_path("static/{**path}").get(static_dir));

        Service::new(router).catcher(Catcher::default().hoop(html::main::handle_errors))
}
