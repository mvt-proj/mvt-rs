use clap::{Arg, Command};
use dotenv;
use salvo::prelude::*;
use sqlx::PgPool;
use std::cell::OnceCell;
use std::path::Path;

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

mod api;
mod auth;
mod cache;
mod catalog;
mod db;
mod health;
mod html;
mod routes;
mod storage;
mod tiles;

use auth::Auth;
use cache::DiskCache;
use catalog::Catalog;
use db::make_db_pool;

#[derive(Debug)]
pub struct AppState {
    db_pool: PgPool,
    catalog: Catalog,
    disk_cache: DiskCache,
    auth: Auth,
    jwt_secret: String,
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


async fn init(config_dir: &str) {
    if !Path::new(&config_dir).exists() {
        std::fs::create_dir(&config_dir).unwrap();
    }
    // Catalog
    let path = format!("{config_dir}/catalog.json");
    let file_path = Path::new(&path);

    if !file_path.exists() {
        let json_str = "[]";
        let mut file = File::create(file_path).await.unwrap();
        file.write_all(json_str.as_bytes()).await.unwrap();
        file.flush().await.unwrap();
    }

    // Users
    let path = format!("{config_dir}/users.json");
    let file_path = Path::new(&path);

    if !file_path.exists() {
        let json_str = "[]";
        let mut file = File::create(file_path).await.unwrap();
        file.write_all(json_str.as_bytes()).await.unwrap();
        file.flush().await.unwrap();
    }
}

#[tokio::main]
async fn main() {
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
            Arg::new("dbconn")
                .short('d')
                .long("dbconn")
                .value_name("DBCONN")
                .required(false)
                .help("Database connection"),
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

    init(config_dir).await;

    dotenv::dotenv().ok();

    let mut db_conn = String::new();
    if matches.contains_id("dbconn") {
        db_conn = matches.get_one::<String>("dbconn").expect("required").to_string();
    }

    let mut jwt_secret = String::new();
    if matches.contains_id("jwtsecret") {
        jwt_secret = matches.get_one::<String>("jwtsecret").expect("required").to_string();
    }

    let host = std::env::var("IPHOST").unwrap_or("127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or("5887".to_string());

    if db_conn.is_empty() {
        db_conn = std::env::var("DBCONN").expect("DBCONN needs to be defined");
    }

    if jwt_secret.is_empty() {
        jwt_secret = std::env::var("JWTSECRET").expect("JWTSECRET needs to be defined");
    }

    let db_pool_size_min = std::env::var("POOLSIZEMIN").unwrap_or("2".to_string());
    let db_pool_size_max = std::env::var("POOLSIZEMAX").unwrap_or("5".to_string());
    let salt_string = std::env::var("SALTSTRING").expect("SALTSTRING needs to be defined");
    let delete_cache = std::env::var("DELETECACHE").unwrap_or("0".to_string());

    let db_pool_size_min: u32 = db_pool_size_min.parse().unwrap();
    let db_pool_size_max: u32 = db_pool_size_max.parse().unwrap();
    let delete_cache: u8 = delete_cache.parse().unwrap();

    let auth = Auth::new(config_dir, salt_string)
        .await
        .expect("The 'auth' structure could not be initialized");

    let catalog = Catalog::new(config_dir)
        .await
        .expect("The 'layers' structure could not be initialized");

    let disk_cache = DiskCache::new(cache_dir.into());
    if delete_cache != 0 {
        disk_cache.delete_cache_dir(catalog.clone()).await;
    }

    let db_pool = match make_db_pool(&db_conn, db_pool_size_min, db_pool_size_max).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Could not connect to the database {}", &db_conn);
            panic!("Database connection error: {}", e);
        }
    };

    let app_state = AppState {
        db_pool,
        catalog,
        disk_cache,
        auth,
        jwt_secret,
    };

    unsafe {
        APP_STATE.set(app_state).unwrap();
    }

    let acceptor = TcpListener::new(format!("{host}:{port}")).bind().await;
    Server::new(acceptor).serve(routes::app_router()).await;
}
