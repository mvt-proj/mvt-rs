use salvo::prelude::*;
use sqlx::{Connection, Executor, PgPool, SqliteConnection, SqlitePool};
use std::cell::OnceCell;
use std::fs;
use std::path::Path;

mod api;
mod args;
mod auth;
mod catalog;
mod database;
mod db;
mod diskcache;
mod error;
mod health;
mod html;
mod rediscache;
mod routes;
mod storage;
mod tiles;

use auth::Auth;
use catalog::Catalog;
use db::make_db_pool;
use diskcache::DiskCache;
use error::AppResult;
use rediscache::RedisCache;

#[derive(Debug)]
pub struct AppState {
    db_pool: PgPool,
    cf_pool: SqlitePool,
    catalog: Catalog,
    disk_cache: DiskCache,
    auth: Auth,
    jwt_secret: String,
    use_redis_cache: bool,
    redis_cache: Option<RedisCache>,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

pub fn get_app_state() -> &'static mut AppState {
    unsafe { APP_STATE.get_mut().unwrap() }
}

pub fn get_db_pool() -> &'static PgPool {
    unsafe { &APP_STATE.get().unwrap().db_pool }
}

pub fn get_catalog() -> &'static Catalog {
    unsafe { &APP_STATE.get().unwrap().catalog }
}

pub fn get_disk_cache() -> &'static DiskCache {
    unsafe { &APP_STATE.get().unwrap().disk_cache }
}

pub fn get_auth() -> &'static Auth {
    unsafe { &APP_STATE.get().unwrap().auth }
}

pub fn get_jwt_secret() -> &'static String {
    unsafe { &APP_STATE.get().unwrap().jwt_secret }
}

async fn initialize_auth(config_dir: &str, salt_string: String) -> AppResult<Auth> {
    let auth = Auth::new(config_dir, salt_string).await?;
    Ok(auth)
}

async fn initialize_catalog(config_dir: &str) -> AppResult<Catalog> {
    let catalog = Catalog::new(config_dir).await?;
    Ok(catalog)
}

async fn initialize_redis_cache(
    redis_conn: String,
    catalog: &Catalog,
) -> AppResult<Option<RedisCache>> {
    if redis_conn.is_empty() {
        return Ok(None);
    }

    let cache = RedisCache::new(redis_conn).await?;
    cache.delete_cache(catalog.clone()).await?;
    Ok(Some(cache))
}

pub async fn init_db(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    if !Path::new(db_path).exists() {
        println!("Database file not found, initializing: {}", db_path);

        if let Some(parent) = Path::new(db_path).parent() {
            fs::create_dir_all(parent).expect("Failed to create database directory");
            fs::File::create(db_path).expect("Failed to create database file");
        }

        let mut conn = SqliteConnection::connect(&format!("sqlite:{}", db_path)).await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        username TEXT NOT NULL,
                        email TEXT NOT NULL UNIQUE,
                        password TEXT NOT NULL,
                        group_ids TEXT NOT NULL 
                    );",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS groups (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL UNIQUE,
                        description TEXT NOT NULL
                    );",
        )
        .await?;

        conn.execute(
            "
            INSERT INTO groups (name, description)
            VALUES ('admin', 'admin role');
        ",
        )
        .await?;

        println!("Database initialized successfully.");
    } else {
        println!("Database file found, skipping initialization.");
    }

    // Crea el pool de conexiones
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path)).await?;
    Ok(pool)
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
    let auth = initialize_auth(&app_config.config_dir, app_config.salt_string).await?;
    let catalog = initialize_catalog(&app_config.config_dir).await?;

    let db_pool = make_db_pool(
        &app_config.db_conn,
        app_config.db_pool_size_min,
        app_config.db_pool_size_max,
    )
    .await?;

    let disk_cache = DiskCache::new(app_config.cache_dir.into());
    disk_cache.delete_cache_dir(catalog.clone()).await;

    let mut use_redis_cache = false;
    let redis_cache = match initialize_redis_cache(app_config.redis_conn, &catalog).await {
        Ok(Some(cache)) => {
            use_redis_cache = true;
            Some(cache)
        }
        Ok(None) => None,
        Err(err) => {
            tracing::error!(
                "Error initializing Redis cache: {}. The disk will be used as cache storage!",
                err
            );
            None
        }
    };

    let db_conn = &format!("{}/mvtrs.db", app_config.config_dir);
    let cf_pool = init_db(db_conn).await?;

    let app_state = AppState {
        db_pool,
        cf_pool,
        catalog,
        disk_cache,
        auth,
        jwt_secret: app_config.jwt_secret,
        use_redis_cache,
        redis_cache,
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
