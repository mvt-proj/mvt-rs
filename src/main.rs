use clap::{Arg, Command};
use salvo::prelude::*;
use sqlx::PgPool;
use std::cell::OnceCell;
use std::path::Path;

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use anyhow::{anyhow, Context};

mod api;
mod auth;
mod cache;
mod catalog;
mod database;
mod db;
mod health;
mod html;
mod rediscache;
mod routes;
mod storage;
mod tiles;

use auth::Auth;
use cache::DiskCache;
use catalog::Catalog;
use db::make_db_pool;
use rediscache::RedisCache;

#[derive(Debug)]
pub struct AppState {
    db_pool: PgPool,
    sql_mode: String,
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

async fn create_config_files(config_dir: &str) -> Result<(), anyhow::Error> {
    let paths_to_create = ["catalog.json", "users.json"];
    for path in paths_to_create.iter() {
        let file_path = Path::new(config_dir).join(path);
        if !file_path.exists() {
            let json_str = "[]";
            let mut file = File::create(file_path).await?;
            file.write_all(json_str.as_bytes()).await?;
            file.flush().await?;
        }
    }
    Ok(())
}

async fn initialize_auth(config_dir: &str, salt_string: String) -> Result<Auth, anyhow::Error> {
    Auth::new(config_dir, salt_string)
        .await
        .map_err(|err| anyhow!("Error initializing 'Auth': {}", err))
        .context("Failed to initialize 'Auth'")
}

async fn initialize_catalog(config_dir: &str) -> Result<Catalog, anyhow::Error> {
    Catalog::new(config_dir)
        .await
        .map_err(|err| anyhow!("Error initializing 'Catalog': {}", err))
        .context("Failed to initialize 'Catalog'")
}

async fn initialize_redis_cache(
    redis_conn: String,
    catalog: &Catalog,
) -> Result<Option<RedisCache>, anyhow::Error> {
    if redis_conn.is_empty() {
        return Ok(None);
    }

    match RedisCache::new(redis_conn).await {
        Ok(cache) => {
            cache
                .delete_cache(catalog.clone())
                .await
                .with_context(|| "Failed to delete cache for catalog".to_string())?;
            Ok(Some(cache))
        }
        Err(e) => {
            tracing::error!("Failed to create Redis cache: {}", e);
            Ok(None)
        }
    }
}
// async fn create_app_state() -> Result<AppState, anyhow::Error> {
// }
//
//
// async fn start_server(app_state: AppState) -> Result<(), anyhow::Error> {
//     // Start the server here...
//     Ok(())
// }
//
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        // .json()
        .with_env_filter("error")
        .with_env_filter("warn")
        // .with_env_filter("info")
        .init();

    let matches = Command::new("mvt-rs vector tiles server")
        .arg(
            Arg::new("configdir")
                .short('c')
                .long("config")
                .value_name("CONFIGDIR")
                .default_value("config")
                .help("Directory where config files are placed"),
        )
        .arg(
            Arg::new("cachedir")
                .short('b')
                .long("cache")
                .value_name("CACHEDIR")
                .default_value("cache")
                .help("Directory where cache files are placed"),
        )
        .arg(
            Arg::new("host")
                .short('i')
                .long("host")
                .value_name("HOST")
                .default_value("0.0.0.0")
                .help("Bind address"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .default_value("5887")
                .help("Bind port"),
        )
        .arg(
            Arg::new("dbconn")
                .short('d')
                .long("dbconn")
                .value_name("DBCONN")
                .required(false)
                .help("Database connection"),
        )
        .arg(
            Arg::new("sqlmode")
                .short('s')
                .long("sqlmode")
                .value_name("SQLMODE")
                .default_value("CTE")
                .help("SQL Query Mode. CTE: Common Table Expression - SQ: Subquery"),
        )
        .arg(
            Arg::new("redisconn")
                .short('r')
                .long("redisconn")
                .value_name("REDISCONN")
                .required(false)
                .help("Redis connection"),
        )
        .arg(
            Arg::new("jwtsecret")
                .short('j')
                .long("jwtsecret")
                .value_name("JWTSECRET")
                .required(false)
                .help("JWT secret key"),
        )
        .get_matches();

    let config_dir = matches.get_one::<String>("configdir").expect("required");
    let cache_dir = matches.get_one::<String>("cachedir").expect("required");

    create_config_files(config_dir).await?;

    dotenv::dotenv().ok();

    let mut host = String::new();
    let mut port = String::new();
    let mut db_conn = String::new();
    let mut redis_conn = String::new();
    let mut sql_mode = String::new();
    let mut jwt_secret = String::new();

    if matches.contains_id("host") {
        host = matches
            .get_one::<String>("host")
            .expect("required")
            .to_string();
    }

    if matches.contains_id("port") {
        port = matches
            .get_one::<String>("port")
            .expect("required")
            .to_string();
    }

    if matches.contains_id("dbconn") {
        db_conn = matches
            .get_one::<String>("dbconn")
            .expect("required")
            .to_string();
    }

    if matches.contains_id("redisconn") {
        redis_conn = matches
            .get_one::<String>("redisconn")
            .expect("required")
            .to_string();
    }

    if matches.contains_id("sqlmode") {
        sql_mode = matches
            .get_one::<String>("sqlmode")
            .expect("required")
            .to_string();
    }

    if matches.contains_id("jwtsecret") {
        jwt_secret = matches
            .get_one::<String>("jwtsecret")
            .expect("required")
            .to_string();
    }


    if host.is_empty() {
        host = std::env::var("IPHOST").expect("IPHOST needs to be defined");
    }

    if port.is_empty() {
        port = std::env::var("PORT").expect("PORT needs to be defined");
    }

    if db_conn.is_empty() {
        db_conn = std::env::var("DBCONN").expect("DBCONN needs to be defined");
    }

    if sql_mode.is_empty() {
        sql_mode = std::env::var("SQLMODE").unwrap_or(String::from("CTE"));
    }
    if redis_conn.is_empty() {
        redis_conn = std::env::var("REDISCONN").unwrap_or_default();
    }

    if jwt_secret.is_empty() {
        jwt_secret = std::env::var("JWTSECRET").expect("JWTSECRET needs to be defined");
    }

    let db_pool_size_min = std::env::var("POOLSIZEMIN").unwrap_or("2".to_string());
    let db_pool_size_max = std::env::var("POOLSIZEMAX").unwrap_or("5".to_string());
    let salt_string = std::env::var("SALTSTRING").expect("SALTSTRING needs to be defined");

    let db_pool_size_min: u32 = db_pool_size_min.parse().unwrap();
    let db_pool_size_max: u32 = db_pool_size_max.parse().unwrap();

    let auth = initialize_auth(config_dir, salt_string)
        .await
        .with_context(|| {
            format!(
                "Failed to initialize 'Auth' for config directory '{}'",
                config_dir
            )
        })?;

    let catalog = initialize_catalog(config_dir).await.with_context(|| {
        format!(
            "Failed to initialize 'Catalog' for config directory '{}'",
            config_dir
        )
    })?;

    let db_pool = make_db_pool(&db_conn, db_pool_size_min, db_pool_size_max).await?;

    let disk_cache = DiskCache::new(cache_dir.into());
    disk_cache.delete_cache_dir(catalog.clone()).await;

    let mut use_redis_cache = false;
    let redis_cache = match initialize_redis_cache(redis_conn, &catalog).await {
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

    let app_state = AppState {
        db_pool,
        sql_mode,
        catalog,
        disk_cache,
        auth,
        jwt_secret,
        use_redis_cache,
        redis_cache,
    };

    unsafe {
        APP_STATE.set(app_state).unwrap();
    }

    let acceptor = TcpListener::new(format!("{host}:{port}")).bind().await;
    Server::new(acceptor).serve(routes::app_router()).await;

    Ok(())
}
