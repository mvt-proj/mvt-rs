use config::categories::get_categories as get_cf_categories;
use salvo::prelude::*;
use sqlx::{PgPool, SqlitePool};
use std::sync::OnceLock;
use tokio::sync::{OnceCell, RwLock};

mod api;
mod args;
mod auth;
mod cachewrapper;
mod cli;
mod config;
mod database;
mod db;
mod diskcache;
mod error;
mod filters;
mod html;
mod i18n;
mod models;
mod monitor;
mod rediscache;
mod routes;
mod services;

use auth::Auth;
use cachewrapper::CacheWrapper;
use db::make_db_pool;
use error::AppResult;
use models::{catalog::Catalog, category::Category};
use monitor::spawn_updater;

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

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt()
        // .json()
        .with_env_filter("error")
        .with_env_filter("warn")
        // .with_env_filter("info")
        .init();

    spawn_updater();

    let app_config = args::parse_args().await?;

    let config_cli = app_config.config_cli;

    if config_cli {
        cli::prompts::start_cli(app_config).unwrap();
        return Ok(());
    }

    let db_conn = &format!("{}/mvtrs.db", app_config.config_dir);
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
    let cache_wrapper = cachewrapper::initialize_cache(
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

    let acceptor = TcpListener::new(format!("{}:{}", app_config.host, app_config.port))
        .bind()
        .await;
    Server::new(acceptor)
        .serve(routes::app_router(&app_config))
        .await;

    Ok(())
}
