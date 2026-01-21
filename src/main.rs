use config::categories::get_categories as get_cf_categories;
use salvo::prelude::*;
use salvo::server::ServerHandle;
use sqlx::{PgPool, SqlitePool};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::signal;
use tokio::sync::{OnceCell, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod args;
mod auth;
mod cache;
mod cli;
mod config;
mod db;
mod error;
mod filters;
mod html;
mod i18n;
mod models;
mod monitor;
mod routes;
mod services;

use auth::Auth;
use cache::cachewrapper::CacheWrapper;
use db::make_db_pool;
use error::AppResult;
use models::{catalog::Catalog, category::Category};
use monitor::start_system_monitor;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static POSTGRES_DB: OnceLock<PgPool> = OnceLock::new();
#[inline]
pub fn get_db_pool() -> &'static PgPool {
    POSTGRES_DB.get().unwrap()
}

static SQLITE_CONF: OnceLock<SqlitePool> = OnceLock::new();
#[inline]
pub fn get_cf_pool() -> &'static SqlitePool {
    SQLITE_CONF.get().unwrap()
}

static MAP_ASSETS_DIR: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_map_assets() -> &'static String {
    MAP_ASSETS_DIR.get().unwrap()
}

static JWT_SECRET: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_jwt_secret() -> &'static String {
    JWT_SECRET.get().unwrap()
}

static CACHE_WRAPPER: OnceLock<CacheWrapper> = OnceLock::new();
#[inline]
pub fn get_cache_wrapper() -> &'static CacheWrapper {
    CACHE_WRAPPER.get().unwrap()
}

static CATALOG: OnceCell<RwLock<Catalog>> = OnceCell::const_new();
#[inline]
pub async fn get_catalog() -> &'static RwLock<Catalog> {
    CATALOG.get().unwrap()
}

static CATEGORIES: OnceCell<RwLock<Vec<Category>>> = OnceCell::const_new();
#[inline]
pub async fn get_categories() -> &'static RwLock<Vec<Category>> {
    CATEGORIES.get().unwrap()
}

static AUTH: OnceCell<RwLock<Auth>> = OnceCell::const_new();
#[inline]
pub async fn get_auth() -> &'static RwLock<Auth> {
    AUTH.get().unwrap()
}

async fn initialize_auth(config_dir: &str, pool: &SqlitePool) -> AppResult<Auth> {
    let auth = Auth::new(config_dir, pool).await?;
    Ok(auth)
}

async fn initialize_catalog(pool: &SqlitePool) -> AppResult<Catalog> {
    let catalog = Catalog::new(pool).await?;
    Ok(catalog)
}

async fn listen_shutdown_signal(handle: ServerHandle) {
    // Wait Shutdown Signal
    let ctrl_c = async {
        // Handle Ctrl+C signal
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        // Handle SIGTERM on Unix systems
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(windows)]
    let terminate = async {
        // Handle Ctrl+C on Windows (alternative implementation)
        signal::windows::ctrl_c()
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    // Wait for either signal to be received
    tokio::select! {
        _ = ctrl_c => println!("ctrl_c signal received"),
        _ = terminate => println!("terminate signal received"),
    };

    // Graceful Shutdown Server
    handle.stop_graceful(None);
}

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mvt_server=info,warn".into()),
        )
        .init();

    tokio::spawn(async {
        start_system_monitor().await;
    });

    let app_config = args::parse_args().await?;

    let config_cli = app_config.config_cli;

    if config_cli {
        cli::prompts::start_cli(app_config).unwrap();
        return Ok(());
    }

    let config_path = Path::new(&app_config.config_dir).join("mvtrs.db");
    let db_conn = config_path.to_str().expect("Invalid configuration path");
    let cf_pool = config::db::init_sqlite(db_conn).await?;

    let auth = initialize_auth(&app_config.config_dir, &cf_pool).await?;
    let catalog = initialize_catalog(&cf_pool).await?;

    let db_pool = make_db_pool(
        &app_config.db_conn,
        app_config.db_pool_size_min,
        app_config.db_pool_size_max,
    )
    .await?;

    let categories = get_cf_categories(Some(&cf_pool)).await?;
    let cache_wrapper = CacheWrapper::initialize_cache(
        Some(app_config.redis_conn.clone()),
        app_config.cache_dir.clone().into(),
        catalog.clone(),
    )
    .await?;

    POSTGRES_DB.set(db_pool).unwrap();
    SQLITE_CONF.set(cf_pool).unwrap();
    MAP_ASSETS_DIR
        .set(app_config.map_assets_dir.clone())
        .unwrap();
    JWT_SECRET.set(app_config.jwt_secret.clone()).unwrap();
    CACHE_WRAPPER.set(cache_wrapper).unwrap();
    CATALOG.set(RwLock::new(catalog)).unwrap();
    CATEGORIES.set(RwLock::new(categories)).unwrap();
    AUTH.set(RwLock::new(auth)).unwrap();

    let i18n_service = Arc::new(i18n::I18n::new());

    let acceptor = TcpListener::new(format!("{}:{}", app_config.host, app_config.port))
        .bind()
        .await;
    let server = Server::new(acceptor);
    let handle = server.handle();

    tokio::spawn(listen_shutdown_signal(handle));

    server
        .serve(routes::app_router(&app_config, i18n_service))
        .await;

    Ok(())
}
