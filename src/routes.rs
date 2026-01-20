use include_dir::{Dir, include_dir};
use mime_guess::from_path;
use salvo::cache::{Cache, MokaStore, RequestIssuer};
use salvo::catcher::Catcher;
use salvo::cors::{self as cors, Cors};
use salvo::logging::Logger;
use salvo::prelude::*;
use salvo::session::CookieStore;
use std::sync::Arc;
use std::time::Duration; // <--- Necesario

use crate::{
    api, args, auth, html,
    i18n::{I18n, i18n_middleware},
    monitor,
    services::{health, legends, styles, tiles::handlers as tiles},
};

const STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/static");

// ============================================================================
// HANDLERS
// ============================================================================

#[handler]
async fn serve_static(req: &mut Request, res: &mut Response) {
    let path = req.uri().path().trim_start_matches("/static/");

    if let Some(file) = STATIC_DIR.get_file(path) {
        let content_type = from_path(path).first_or_octet_stream().to_string();

        if let Ok(header_value) = content_type.parse() {
            res.headers_mut().insert("Content-Type", header_value);
        } else if let Ok(fallback_value) = "application/octet-stream".parse() {
            res.headers_mut().insert("Content-Type", fallback_value);
        }

        let _ = res.write_body(file.contents());
    } else {
        res.status_code(StatusCode::NOT_FOUND);
    }
}

// ============================================================================
// MIDDLEWARE BUILDERS
// ============================================================================

fn build_cors_handler() -> impl Handler + Clone {
    Cors::new()
        .allow_origin(cors::Any)
        .allow_methods(cors::Any)
        .allow_headers(cors::Any)
        .into_handler()
}

fn build_session_handler(app_config: &args::AppConfig) -> SessionHandler<CookieStore> {
    SessionHandler::builder(CookieStore::new(), app_config.session_secret.as_bytes())
        .session_ttl(Some(Duration::from_secs(60 * 20))) // 20 minutos
        .build()
        .expect("Failed to build session handler")
}

fn build_cache_middleware(ttl_secs: u64) -> Cache<MokaStore<String>, RequestIssuer> {
    Cache::new(
        MokaStore::builder()
            .time_to_live(Duration::from_secs(ttl_secs))
            .build(),
        RequestIssuer::default(),
    )
}

// ============================================================================
// ROUTE BUILDERS
// ============================================================================

fn build_auth_routes() -> Router {
    Router::new()
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
}

fn build_protected_pages() -> Router {
    Router::new()
        .hoop(auth::session_auth_handler)
        .push(Router::with_path("catalog").get(html::main::page_catalog))
        .push(Router::with_path("catalogtable").get(html::main::table_catalog))
        .push(Router::with_path("styles").get(html::main::page_styles))
        .push(Router::with_path("styletable").get(html::main::table_styles))
        .push(Router::with_path("sprites").get(html::main::page_sprites))
        .push(Router::with_path("glyphs").get(html::main::page_glyphs))
        .push(Router::with_path("maplayer/{layer_name}").get(html::main::page_map_layer))
        .push(Router::with_path("mapview/{style_id}").get(html::main::page_map_view))
}

fn build_admin_users_routes() -> Router {
    Router::with_path("users")
        .hoop(auth::require_user_admin)
        .get(html::admin::users::list_users)
        .push(Router::with_path("new").get(html::admin::main::new_user))
        .push(Router::with_path("create").post(html::admin::users::create_user))
        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_user))
        .push(Router::with_path("update").post(html::admin::users::update_user))
        .push(Router::with_path("delete/{id}").get(html::admin::users::delete_user))
}

fn build_admin_categories_routes() -> Router {
    Router::with_path("categories")
        .hoop(auth::require_user_admin)
        .get(html::admin::categories::list_categories)
        .push(Router::with_path("new").get(html::admin::main::new_category))
        .push(Router::with_path("create").post(html::admin::categories::create_category))
        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_category))
        .push(Router::with_path("update").post(html::admin::categories::edit_category))
        .push(Router::with_path("delete/{id}").get(html::admin::categories::delete_category))
}

fn build_admin_styles_routes() -> Router {
    Router::with_path("styles")
        .hoop(auth::require_user_admin)
        .get(html::admin::styles::list_styles)
        .push(Router::with_path("new").get(html::admin::main::new_style))
        .push(Router::with_path("create").post(html::admin::styles::create_style))
        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_style))
        .push(Router::with_path("update").post(html::admin::styles::edit_style))
        .push(Router::with_path("delete/{id}").get(html::admin::styles::delete_style))
}

fn build_admin_groups_routes() -> Router {
    Router::with_path("groups")
        .hoop(auth::require_user_admin)
        .get(html::admin::groups::list_groups)
        .push(Router::with_path("new").get(html::admin::main::new_group))
        .push(Router::with_path("create").post(html::admin::groups::create_group))
        .push(Router::with_path("edit/{id}").get(html::admin::main::edit_group))
        .push(Router::with_path("update").post(html::admin::groups::edit_group))
        .push(Router::with_path("delete/{id}").get(html::admin::groups::delete_group))
}

fn build_admin_catalog_routes() -> Router {
    Router::with_path("catalog")
        .hoop(auth::require_user_admin)
        .get(html::admin::catalog::page_catalog)
        .push(Router::with_path("layers/new").get(html::admin::main::new_layer))
        .push(Router::with_path("layers/create").post(html::admin::catalog::create_layer))
        .push(Router::with_path("layers/edit/{id}").get(html::admin::main::edit_layer))
        .push(Router::with_path("layers/delete/{id}").get(html::admin::catalog::delete_layer))
        .push(Router::with_path("layers/update").post(html::admin::catalog::update_layer))
        .push(
            Router::with_path("layers/swichpublished/{id}")
                .get(html::admin::catalog::swich_published),
        )
        .push(
            Router::with_path("layers/delete_cache/{id}")
                .get(html::admin::catalog::delete_layer_cache),
        )
}

fn build_admin_database_routes() -> Router {
    Router::with_path("database")
        .push(Router::with_path("schemas").get(html::admin::database::schemas))
        .push(Router::with_path("tables").get(html::admin::database::tables))
        .push(Router::with_path("fields").get(html::admin::database::fields))
        .push(Router::with_path("srid").get(html::admin::database::srid))
}

fn build_admin_monitor_routes() -> Router {
    Router::with_path("monitor")
        .push(Router::with_path("dashboard").get(monitor::handlers::dashboard))
        .push(Router::with_path("ssemetrics").get(monitor::handlers::sse_metrics))
}

fn build_admin_routes() -> Router {
    Router::with_path("admin")
        .hoop(auth::session_auth_handler)
        .get(html::admin::main::index)
        .push(build_admin_users_routes())
        .push(build_admin_categories_routes())
        .push(build_admin_styles_routes())
        .push(build_admin_groups_routes())
        .push(build_admin_catalog_routes())
        .push(build_admin_database_routes())
        .push(build_admin_monitor_routes())
}

fn build_api_users_routes() -> Router {
    Router::with_path("users")
        .hoop(auth::validate_token)
        .get(api::users::index)
        .post(api::users::create)
}

fn build_api_database_routes() -> Router {
    Router::with_path("database")
        .hoop(auth::validate_token)
        .push(Router::with_path("schemas").get(api::database::schemas))
        .push(Router::with_path("tables/{schema}").get(api::database::tables))
        .push(Router::with_path("fields/{schema}/{table}").get(api::database::fields))
        .push(Router::with_path("srid/{schema}/{table}/{geometry}").get(api::database::srid))
}

fn build_api_catalog_routes() -> Router {
    Router::with_path("catalog/layer")
        .hoop(auth::validate_token)
        .get(api::catalog::list)
        .post(api::catalog::create_layer)
}

fn build_api_routes(cors_handler: impl Handler + Clone) -> Router {
    Router::with_path("api")
        .hoop(cors_handler.clone())
        .push(
            Router::with_path("users/login")
                .post(api::users::login)
                .options(handler::empty()),
        )
        .push(Router::with_path("monitor/metrics").get(monitor::handlers::metrics))
        .push(Router::with_path("catalog/layer").get(api::catalog::list))
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .push(build_api_users_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes())
                .push(
                    Router::with_path("{**rest}")
                        .hoop(cors_handler)
                        .options(handler::empty()),
                ),
        )
}

fn build_tiles_routes() -> Router {
    Router::new()
        .push(
            Router::with_path("tiles/{layer_name}/{z}/{x}/{y}.pbf")
                .get(tiles::get_single_layer_tile),
        )
        .push(
            Router::with_path("tiles/multi/{layers}/{z}/{x}/{y}.pbf")
                .get(tiles::get_composite_layers_tile),
        )
        .push(
            Router::with_path("tiles/category/{category}/{z}/{x}/{y}.pbf")
                .get(tiles::get_category_layers_tile),
        )
}

fn build_services_routes(
    app_config: &args::AppConfig,
    cache: impl Handler,
    cors_handler: impl Handler,
) -> Router {
    Router::with_path("services")
        .hoop(cache)
        .hoop(cors_handler)
        .options(handler::empty())
        .push(build_tiles_routes())
        .push(Router::with_path("styles/{style_name}").get(styles::index))
        .push(Router::with_path("legends/{style_name}").get(legends::index))
        .push(
            Router::with_path("map_assets/{**path}").get(
                StaticDir::new([&app_config.map_assets_dir])
                    .include_dot_files(false)
                    .defaults("index.html")
                    .auto_list(true),
            ),
        )
}

fn build_public_routes() -> Router {
    Router::new()
        .hoop(i18n_middleware)
        .get(html::main::index)
        .push(build_auth_routes())
        .push(build_protected_pages())
        .push(build_admin_routes())
}

// ============================================================================
// MAIN ROUTER
// ============================================================================

pub fn app_router(app_config: &args::AppConfig, i18n_service: Arc<I18n>) -> Service {
    let cache_5s = build_cache_middleware(5);
    let cors_handler = build_cors_handler();
    let session_handler = build_session_handler(app_config);

    let router = Router::new()
        .hoop(Logger::default())
        .hoop(affix_state::inject(i18n_service))
        .hoop(session_handler)
        .push(build_public_routes())
        .push(build_api_routes(cors_handler.clone()))
        .push(Router::with_path("health").get(health::get_health))
        .push(build_services_routes(app_config, cache_5s, cors_handler))
        .push(Router::with_path("static/{**path}").get(serve_static));

    Service::new(router).catcher(Catcher::default().hoop(html::main::handle_errors))
}
