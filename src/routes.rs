use include_dir::{Dir, include_dir};
use mime_guess::from_path;
use salvo::cache::{Cache, MokaStore, RequestIssuer};
use salvo::catcher::Catcher;
use salvo::cors::{self as cors, Cors};
use salvo::http::header::CONTENT_DISPOSITION;
use salvo::logging::Logger;
use salvo::prelude::*;
use salvo::rate_limiter::{BasicQuota, FixedGuard, MokaStore as RateMokaStore, RateLimiter, RemoteIpIssuer};
use salvo::session::{CookieStore, SessionHandler};
use std::sync::Arc;
use std::time::Duration;

use crate::{
    api, auth, config::settings::Settings, html,
    i18n::{I18n, i18n_middleware},
    monitor,
    services::{health, legends, styles, tilejson, tiles::handlers as tiles},
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
        .expose_headers(vec![CONTENT_DISPOSITION])
        .allow_credentials(false)
        .into_handler()
}

fn build_session_handler(settings: &Settings) -> SessionHandler<CookieStore> {
    SessionHandler::builder(CookieStore::new(), settings.security.session_secret.as_bytes())
        .session_ttl(Some(Duration::from_secs(60 * settings.security.session_duration_minutes)))
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

fn build_login_rate_limiter() -> impl Handler {
    RateLimiter::new(
        FixedGuard::new(),
        RateMokaStore::new(),
        RemoteIpIssuer,
        BasicQuota::per_minute(10),
    )
}

// ============================================================================
// ROUTE BUILDERS
// ============================================================================

fn build_auth_routes() -> Router {
    Router::new()
        .push(Router::with_path("login").get(html::pages::login))
        .push(
            Router::with_path("logout")
                .hoop(auth::session_auth_handler)
                .get(auth::logout),
        )
        .push(
            Router::with_path("auth/login")
                .hoop(build_login_rate_limiter())
                .post(auth::login),
        )
        .push(
            Router::with_path("changepassword")
                .hoop(auth::session_auth_handler)
                .get(html::pages::change_password),
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
        .push(Router::with_path("catalog").get(html::catalog::page_catalog))
        .push(Router::with_path("catalogtable").get(html::catalog::table_catalog))
        .push(Router::with_path("styles").get(html::styles::page_styles))
        .push(Router::with_path("styletable").get(html::styles::table_styles))
        .push(Router::with_path("sprites").get(html::assets::page_sprites))
        .push(Router::with_path("glyphs").get(html::assets::page_glyphs))
        .push(Router::with_path("maplayer/{layer_name}").get(html::maps::page_map_layer))
        .push(Router::with_path("mapview/{style_id}").get(html::maps::page_map_view))
}

fn build_admin_users_routes() -> Router {
    Router::with_path("users")
        .hoop(auth::require_user_admin)
        .get(html::admin::users::list_users)
        .push(Router::with_path("new").get(html::admin::users::new_user_page))
        .push(Router::with_path("create").post(html::admin::users::create_user))
        .push(Router::with_path("edit/{id}").get(html::admin::users::edit_user_page))
        .push(Router::with_path("update").post(html::admin::users::update_user))
        .push(Router::with_path("delete/{id}").get(html::admin::users::delete_user))
}

fn build_admin_categories_routes() -> Router {
    Router::with_path("categories")
        .hoop(auth::require_user_admin)
        .get(html::admin::categories::list_categories)
        .push(Router::with_path("new").get(html::admin::categories::new_category_page))
        .push(Router::with_path("create").post(html::admin::categories::create_category))
        .push(Router::with_path("edit/{id}").get(html::admin::categories::edit_category_page))
        .push(Router::with_path("update").post(html::admin::categories::update_category))
        .push(Router::with_path("delete/{id}").get(html::admin::categories::delete_category))
}

// QML/SLD files (categorized/graduated renderers, label-heavy styles) routinely
// exceed Salvo's global 64 KB secure-max-size default; real-world samples run
// 70-400 KB. Only these routes get a raised limit — the global default is left
// untouched for every other endpoint.
const STYLE_IMPORT_MAX_BODY_SIZE: usize = 8 * 1024 * 1024;

fn build_admin_styles_routes() -> Router {
    Router::with_path("styles")
        .hoop(auth::require_user_admin)
        .get(html::admin::styles::list_styles)
        .push(Router::with_path("table").get(html::styles::table_styles_admin))
        .push(Router::with_path("new").get(html::admin::styles::new_style_page))
        .push(Router::with_path("create").post(html::admin::styles::create_style))
        .push(Router::with_path("edit/{id}").get(html::admin::styles::edit_style_page))
        .push(Router::with_path("update").post(html::admin::styles::update_style))
        .push(Router::with_path("delete/{id}").get(html::admin::styles::delete_style))
        .push(
            Router::with_path("convert-qml")
                .hoop(salvo::http::request::SecureMaxSize::new(
                    STYLE_IMPORT_MAX_BODY_SIZE,
                ))
                .post(html::admin::styles::convert_qml),
        )
        .push(
            Router::with_path("convert-sld")
                .hoop(salvo::http::request::SecureMaxSize::new(
                    STYLE_IMPORT_MAX_BODY_SIZE,
                ))
                .post(html::admin::styles::convert_sld),
        )
}

fn build_admin_groups_routes() -> Router {
    Router::with_path("groups")
        .hoop(auth::require_user_admin)
        .get(html::admin::groups::list_groups)
        .push(Router::with_path("new").get(html::admin::groups::new_group_page))
        .push(Router::with_path("create").post(html::admin::groups::create_group))
        .push(Router::with_path("edit/{id}").get(html::admin::groups::edit_group_page))
        .push(Router::with_path("update").post(html::admin::groups::update_group))
        .push(Router::with_path("delete/{id}").get(html::admin::groups::delete_group))
}

/// Metadata is keyed by `layer_id` (1:1 with a `Layer`, design decision #1),
/// not a freestanding entity id — `new`/`edit` both take `{layer_id}`; see
/// `html::admin::metadata` for how the two pages redirect into each other
/// when a record does/doesn't exist yet. Gated by
/// `require_user_metadata_admin` (accepts `admin` or `admin_metadata`,
/// design decision #8), not the stricter `require_user_admin`.
fn build_admin_metadata_routes() -> Router {
    Router::with_path("metadata")
        .hoop(auth::handlers::require_user_metadata_admin)
        .get(html::admin::metadata::list_metadata)
        .push(Router::with_path("table").get(html::admin::metadata::table_metadata))
        .push(Router::with_path("new/{layer_id}").get(html::admin::metadata::new_metadata_page))
        .push(Router::with_path("create").post(html::admin::metadata::create_metadata))
        .push(Router::with_path("edit/{layer_id}").get(html::admin::metadata::edit_metadata_page))
        .push(Router::with_path("update").post(html::admin::metadata::update_metadata))
        .push(Router::with_path("delete/{layer_id}").get(html::admin::metadata::delete_metadata))
}

fn build_admin_catalog_routes() -> Router {
    Router::with_path("catalog")
        .hoop(auth::require_user_admin)
        .get(html::admin::catalog::catalog_page)
        .push(Router::with_path("table").get(html::catalog::table_catalog_admin))
        .push(Router::with_path("layers/new").get(html::admin::catalog::new_layer_page))
        .push(Router::with_path("layers/create").post(html::admin::catalog::create_layer))
        .push(Router::with_path("layers/edit/{id}").get(html::admin::catalog::edit_layer_page))
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
        .push(Router::with_path("spatial_index").get(html::admin::database::spatial_index))
}

fn build_admin_monitor_routes() -> Router {
    Router::with_path("monitor")
        .push(Router::with_path("dashboard").get(monitor::handlers::dashboard))
        .push(Router::with_path("ssemetrics").get(monitor::handlers::sse_metrics))
}

fn build_admin_routes() -> Router {
    Router::with_path("admin")
        .hoop(auth::session_auth_handler)
        .get(html::admin::dashboard::index)
        .push(build_admin_users_routes())
        .push(build_admin_categories_routes())
        .push(build_admin_styles_routes())
        .push(build_admin_groups_routes())
        .push(build_admin_metadata_routes())
        .push(build_admin_catalog_routes())
        .push(build_admin_database_routes())
        .push(build_admin_monitor_routes())
        .push(Router::with_path("plugins").get(html::admin::plugins::index))
}

fn build_api_users_routes() -> Router {
    Router::with_path("users")
        .get(api::users::index)
        .post(api::users::create)
        .push(
            Router::with_path("{id}")
                .put(api::users::update)
                .delete(api::users::delete),
        )
}

fn build_api_groups_routes() -> Router {
    Router::with_path("groups")
        .get(api::groups::list)
        .post(api::groups::create)
        .push(
            Router::with_path("{id}")
                .put(api::groups::update)
                .delete(api::groups::delete),
        )
}

fn build_api_categories_routes() -> Router {
    Router::with_path("categories")
        .get(api::categories::list)
        .post(api::categories::create)
        .push(
            Router::with_path("{id}")
                .put(api::categories::update)
                .delete(api::categories::delete),
        )
}

fn build_api_styles_routes() -> Router {
    Router::with_path("styles")
        .get(api::styles::list)
        .post(api::styles::create)
        .push(
            Router::with_path("{id}")
                .put(api::styles::update)
                .delete(api::styles::delete),
        )
}

fn build_api_database_routes() -> Router {
    Router::with_path("database")
        .push(Router::with_path("schemas").get(api::database::schemas))
        .push(Router::with_path("tables/{schema}").get(api::database::tables))
        .push(Router::with_path("fields/{schema}/{table}").get(api::database::fields))
        .push(Router::with_path("srid/{schema}/{table}/{geometry}").get(api::database::srid))
        .push(
            Router::with_path("spatial_index/{schema}/{table}/{geometry}")
                .get(api::database::spatial_index),
        )
}

fn build_api_catalog_routes() -> Router {
    Router::with_path("catalog/layer")
        .get(api::catalog::list)
        .post(api::catalog::create_layer)
        .push(
            Router::with_path("{id}")
                .put(api::catalog::update_layer)
                .delete(api::catalog::delete_layer)
                .push(Router::with_path("publish").patch(api::catalog::toggle_published))
                .push(Router::with_path("cache").delete(api::catalog::delete_layer_cache)),
        )
}

/// Admin CRUD JSON for a layer's metadata record (Phase 3 handlers, wired
/// here). Deliberately NOT nested under the `admin` group below: that group
/// hoops the stricter `require_api_admin` (accepts only `admin`), while
/// metadata write ops must also accept `admin_metadata` (design decision
/// #8) — hence its own sibling hoop chain with `require_api_metadata_admin`.
fn build_api_metadata_routes() -> Router {
    Router::with_path("metadata/{layer_id}")
        .hoop(auth::jwt_auth_handler())
        .hoop(auth::validate_token)
        .hoop(auth::handlers::require_api_metadata_admin)
        .get(api::metadata::get)
        .post(api::metadata::create)
        .put(api::metadata::update)
        .delete(api::metadata::delete)
}

fn build_api_routes() -> Router {
    Router::with_path("api")
        .push(
            Router::with_path("users/login")
                .hoop(build_login_rate_limiter())
                .post(api::users::login),
        )
        .push(Router::with_path("monitor/metrics").get(monitor::handlers::metrics))
        .push(Router::with_path("catalog/layer").get(api::catalog::list))
        .push(build_api_metadata_routes())
        .push(
            Router::with_path("admin")
                .hoop(auth::jwt_auth_handler())
                .hoop(auth::validate_token)
                .hoop(auth::require_api_admin)
                .push(build_api_users_routes())
                .push(build_api_groups_routes())
                .push(build_api_categories_routes())
                .push(build_api_styles_routes())
                .push(build_api_database_routes())
                .push(build_api_catalog_routes()),
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

/// OGC API - Records discovery (Phase 3 handlers, design decision #11:
/// `/services/records` is mvt-rs's public read surface, mirroring the rest
/// of `/services/*`; deliberately NOT behind the metadata-admin hoop —
/// visibility is enforced per-item by `api::metadata::items` itself
/// (published + `validate_user_groups`, spec "Discovery respects visibility
/// rules"), the same pattern `tilejson_index` already uses.
fn build_records_routes() -> Router {
    let router = Router::with_path("records")
        .get(api::metadata::landing)
        .push(Router::with_path("conformance").get(api::metadata::conformance))
        .push(Router::with_path("collections").get(api::metadata::collections))
        .push(Router::with_path("collections/{collection_id}").get(api::metadata::collection))
        .push(Router::with_path("collections/{collection_id}/items").get(api::metadata::items))
        .push(Router::with_path("collections/{collection_id}/items/{id}").get(api::metadata::item));

    // `merge_router` walks `router`'s own tree, which already starts at the
    // "records" segment — base "/services" (not "/services/records") is
    // what makes the generated document's `paths` match the real mount
    // point under `build_services_routes`. Closes the Geonovum
    // `[unrecognized-format]` gap: `landing`'s `service-desc` link
    // (`api::metadata::build_landing`) points at this route.
    let openapi = OpenApi::new("MVT Server — OGC API - Records", env!("CARGO_PKG_VERSION"))
        .merge_router_with_base(&router, "/services");

    router
        .push(openapi.into_router("openapi"))
        .push(Scalar::new("/services/records/openapi").title("MVT Server — OGC API - Records").into_router("scalar"))
}

fn build_services_routes(settings: &Settings, cache: impl Handler) -> Router {
    Router::with_path("services")
        .hoop(cache)
        .push(build_tiles_routes())
        .push(build_records_routes())
        .push(Router::with_path("styles/{style_name}").get(styles::index))
        .push(Router::with_path("legends/{style_name}").get(legends::index))
        .push(Router::with_path("tilejson").get(tilejson::tilejson_index))
        .push(Router::with_path("tilejson/{layer_name}.json").get(tilejson::tilejson_layer))
        .push(
            Router::with_path("map_assets/{**path}").get(
                StaticDir::new([&settings.paths.assets])
                    .include_dot_files(false)
                    .defaults("index.html")
                    .auto_list(true),
            ),
        )
}

fn build_public_routes() -> Router {
    Router::new()
        .hoop(i18n_middleware)
        .get(html::pages::index)
        .push(build_auth_routes())
        .push(build_protected_pages())
        .push(build_admin_routes())
}

// ============================================================================
// MAIN ROUTER
// ============================================================================

/// Strips `Set-Cookie` from tile responses at the Service level, which runs
/// its after-phase AFTER the router (including session_handler) has finished.
/// This lets browsers cache tiles while keeping session auth intact for tiles
/// that require group-based access control.
#[handler]
async fn strip_tile_cookie(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    ctrl.call_next(req, depot, res).await;
    if req.uri().path().starts_with("/services/tiles/") {
        res.headers_mut().remove("set-cookie");
    }
}

/// Reduced router for `cluster.mode = client`: no SQLite, so only memory-served
/// reads (tiles/styles/legends), health, and static assets are mounted. Admin,
/// write API, and `/internal` are intentionally absent (nginx routes those to
/// the owner).
pub fn client_app_router(settings: &Settings, i18n_service: Arc<I18n>) -> Service {
    let cache_5s = build_cache_middleware(5);
    let cors_handler = build_cors_handler();
    let session_handler = build_session_handler(settings);

    let router = Router::new()
        .options(handler::empty())
        .hoop(Logger::default())
        .hoop(affix_state::inject(i18n_service))
        .hoop(session_handler)
        .push(Router::with_path("health").get(health::get_health))
        .push(build_services_routes(settings, cache_5s))
        .push(Router::with_path("static/{**path}").get(serve_static));

    Service::new(router)
        .hoop(strip_tile_cookie)
        .hoop(cors_handler)
        .catcher(Catcher::default().hoop(html::errors::handle_errors))
}

pub fn app_router(settings: &Settings, i18n_service: Arc<I18n>) -> Service {
    if settings.cluster.mode == "client" {
        return client_app_router(settings, i18n_service);
    }

    let cache_5s = build_cache_middleware(5);
    let cors_handler = build_cors_handler();
    let session_handler = build_session_handler(settings);

    let mut router = Router::new()
        .options(handler::empty()) // Catch-all OPTIONS para preflight
        .hoop(Logger::default())
        .hoop(affix_state::inject(i18n_service))
        .hoop(session_handler)
        .push(build_public_routes())
        .push(build_api_routes())
        .push(Router::with_path("health").get(health::get_health))
        .push(build_services_routes(settings, cache_5s))
        .push(Router::with_path("static/{**path}").get(serve_static));

    if settings.cluster.mode == "owner" {
        router = router.push(crate::cluster::api::build_internal_routes());
    }

    Service::new(router)
        .hoop(strip_tile_cookie) // outermost: after-phase runs after session_handler
        .hoop(cors_handler)
        .catcher(Catcher::default().hoop(html::errors::handle_errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use salvo::test::{ResponseExt, TestClient};

    // `build_records_routes()` is exercised standalone here (no `/services`
    // prefix), same as the discovery-route tests in `api::metadata` — the
    // OpenAPI document itself is built with base path `/services` (see
    // `build_records_routes`) so its `paths` keys already carry the full
    // public path even though this test hits the router at its own root.
    #[tokio::test]
    async fn records_openapi_route_returns_a_valid_document_with_expected_paths() {
        let service = Service::new(build_records_routes());
        let mut res = TestClient::get("http://127.0.0.1:5800/records/openapi").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);

        let body: serde_json::Value = res.take_json().await.unwrap();
        assert!(body["openapi"].is_string(), "must be a real OpenAPI document");

        let paths = body["paths"].as_object().expect("paths must be an object");
        for expected in [
            "/services/records",
            "/services/records/conformance",
            "/services/records/collections",
            "/services/records/collections/{collection_id}",
            "/services/records/collections/{collection_id}/items",
            "/services/records/collections/{collection_id}/items/{id}",
        ] {
            assert!(paths.contains_key(expected), "expected path '{expected}' in the OpenAPI document");
        }
    }

    // Guards against `salvo_oapi`'s "parameters information not provided"
    // startup warning: every `{placeholder}` in a path must be echoed back
    // in that operation's declared `parameters`, or the generated document
    // is spec-incomplete (OpenAPI requires path params to be documented).
    #[tokio::test]
    async fn records_openapi_route_documents_every_path_parameter() {
        let service = Service::new(build_records_routes());
        let mut res = TestClient::get("http://127.0.0.1:5800/records/openapi").send(&service).await;
        let body: serde_json::Value = res.take_json().await.unwrap();
        let paths = body["paths"].as_object().expect("paths must be an object");

        let cases: &[(&str, &[&str])] = &[
            ("/services/records/collections/{collection_id}", &["collection_id"]),
            ("/services/records/collections/{collection_id}/items", &["collection_id"]),
            ("/services/records/collections/{collection_id}/items/{id}", &["collection_id", "id"]),
        ];

        for (path, expected_params) in cases {
            let get_op = &paths[*path]["get"];
            let declared: Vec<&str> = get_op["parameters"]
                .as_array()
                .map(|params| params.iter().filter_map(|p| p["name"].as_str()).collect())
                .unwrap_or_default();

            for expected in *expected_params {
                assert!(
                    declared.contains(expected),
                    "path '{path}' must declare parameter '{expected}', got {declared:?}"
                );
            }
        }
    }

    // Guards against `salvo_oapi`'s companion "information for not exist
    // parameters" warning: a declared parameter whose `in` defaults to
    // `path` but whose name is NOT one of the path's `{placeholder}`s is
    // flagged as bogus. `items`' filters (`q`, `bbox`, `datetime`, `limit`,
    // `offset`) are query params and must say so explicitly.
    #[tokio::test]
    async fn items_query_filters_are_declared_as_query_parameters() {
        let service = Service::new(build_records_routes());
        let mut res = TestClient::get("http://127.0.0.1:5800/records/openapi").send(&service).await;
        let body: serde_json::Value = res.take_json().await.unwrap();

        let get_op = &body["paths"]["/services/records/collections/{collection_id}/items"]["get"];
        let params = get_op["parameters"].as_array().expect("parameters must be an array");

        for name in ["q", "bbox", "datetime", "limit", "offset"] {
            let param = params
                .iter()
                .find(|p| p["name"] == name)
                .unwrap_or_else(|| panic!("expected declared parameter '{name}'"));
            assert_eq!(param["in"], "query", "parameter '{name}' must be declared `in: query`, got {param}");
        }
    }
}
