use config::categories::get_categories as get_cf_categories;
use salvo::prelude::*;
use sqlx::{PgPool, SqlitePool};
use std::cell::OnceCell;


mod api;
mod args;
mod auth;
mod config;
mod database;
mod db;
mod diskcache;
mod error;
mod health;
mod html;
mod models;
mod rediscache;
mod routes;
mod services;
mod cachewrapper;

use auth::Auth;
use db::make_db_pool;
use error::AppResult;
use models::{catalog::Catalog, category::Category};
use cachewrapper::CacheWrapper;

#[derive(Debug)]
pub struct AppState {
    db_pool: PgPool,
    cf_pool: SqlitePool,
    catalog: Catalog,
    cache_wrapper: CacheWrapper,
    auth: Auth,
    jwt_secret: String,
    categories: Vec<Category>,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

pub fn get_app_state() -> &'static mut AppState {
    unsafe { APP_STATE.get_mut().unwrap() }
}

pub fn get_db_pool() -> &'static PgPool {
    unsafe { &APP_STATE.get().unwrap().db_pool }
}

pub fn get_cf_pool() -> &'static SqlitePool {
    unsafe { &APP_STATE.get().unwrap().cf_pool }
}

pub fn get_catalog() -> &'static Catalog {
    unsafe { &APP_STATE.get().unwrap().catalog }
}

pub fn get_cache_wrapper() -> &'static CacheWrapper {
    unsafe { &APP_STATE.get().unwrap().cache_wrapper }
}

pub fn get_auth() -> &'static Auth {
    unsafe { &APP_STATE.get().unwrap().auth }
}

pub fn get_jwt_secret() -> &'static String {
    unsafe { &APP_STATE.get().unwrap().jwt_secret }
}

pub fn get_categories() -> &'static Vec<Category> {
    unsafe { &APP_STATE.get().unwrap().categories }
}

async fn initialize_auth(
    config_dir: &str,
    salt_string: String,
    pool: &SqlitePool,
) -> AppResult<Auth> {
    let auth = Auth::new(config_dir, salt_string, pool).await?;
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

    let app_config = args::parse_args().await?;

    let db_conn = &format!("{}/mvtrs.db", app_config.config_dir);
    let salt = app_config.salt_string.clone();
    let cf_pool = config::db::init_sqlite(db_conn, salt).await?;

    let auth = initialize_auth(
        &app_config.config_dir,
        app_config.salt_string.clone(),
        &cf_pool,
    )
    .await?;
    let catalog = initialize_catalog(&cf_pool).await?;

    let db_pool = make_db_pool(
        &app_config.db_conn,
        app_config.db_pool_size_min,
        app_config.db_pool_size_max,
    )
    .await?;

    let categories = get_cf_categories(Some(&cf_pool)).await?;
    let cache_wrapper = cachewrapper::initialize_cache(
        Some(app_config.redis_conn),
        app_config.cache_dir.clone().into(),
        catalog.clone(),
    ).await?;

    let app_state = AppState {
        db_pool,
        cf_pool,
        catalog,
        auth,
        jwt_secret: app_config.jwt_secret,
        cache_wrapper,
        categories,
    };

    unsafe {
        APP_STATE.set(app_state).unwrap();
    }

    let acceptor = TcpListener::new(format!("{}:{}", app_config.host, app_config.port))
        .bind()
        .await;
    Server::new(acceptor).serve(routes::app_router()).await;

    Ok(())
}
