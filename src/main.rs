use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
// use category::Category;
use config::categories::get_categories as get_cf_categories;
use salvo::prelude::*;
use sqlx::{Connection, Executor, PgPool, SqliteConnection, SqlitePool};
use std::cell::OnceCell;
use std::fs;
use std::path::Path;
use uuid::Uuid;

mod api;
mod args;
mod auth;
// mod catalog;
// mod category;
mod config;
mod database;
mod db;
mod diskcache;
mod error;
mod health;
mod html;
mod rediscache;
mod routes;
mod models;
mod services;
// mod styles;
mod tiles;

use auth::Auth;
// use catalog::Catalog;
use models::{catalog::Catalog, category::Category};
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

pub fn get_disk_cache() -> &'static DiskCache {
    unsafe { &APP_STATE.get().unwrap().disk_cache }
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

pub async fn init_sqlite(db_path: &str, salt: String) -> Result<SqlitePool, sqlx::Error> {
    if !Path::new(db_path).exists() {
        println!("Database file not found, initializing: {}", db_path);

        if let Some(parent) = Path::new(db_path).parent() {
            fs::create_dir_all(parent).expect("Failed to create database directory");
            fs::File::create(db_path).expect("Failed to create database file");
        }

        let mut conn = SqliteConnection::connect(&format!("sqlite:{}", db_path)).await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                        id TEXT PRIMARY KEY NOT NULL,
                        username TEXT NOT NULL,
                        email TEXT NOT NULL UNIQUE,
                        password TEXT NOT NULL,
                        groups TEXT NOT NULL
                    );",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS categories (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL UNIQUE,
                        description TEXT NOT NULL
                    );",
        )
        .await?;

        let public_category_id = Uuid::new_v4().to_string();

        conn.execute(
            format!(
                "
            INSERT INTO categories (id, name, description)
            VALUES ('{}', 'public', 'public category');
        ",
                public_category_id
            )
            .as_str(),
        )
        .await?;

        conn.execute(
            "
            CREATE TABLE layers (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                geometry TEXT NOT NULL,
                name TEXT NOT NULL,
                alias TEXT NOT NULL,
                schema TEXT NOT NULL,
                table_name TEXT NOT NULL,
                fields TEXT NOT NULL,
                filter TEXT,
                srid INTEGER,
                geom TEXT,
                sql_mode TEXT,
                buffer INTEGER,
                extent INTEGER,
                zmin INTEGER,
                zmax INTEGER,
                zmax_do_not_simplify INTEGER,
                buffer_do_not_simplify INTEGER,
                extent_do_not_simplify INTEGER,
                clip_geom BOOLEAN,
                delete_cache_on_start BOOLEAN,
                max_cache_age INTEGER,
                published BOOLEAN NOT NULL,
                url TEXT
            );
        ",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS groups (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL UNIQUE,
                        description TEXT NOT NULL
                    );",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS styles (
                        id TEXT PRIMARY KEY NOT NULL,
                        category TEXT NOT NULL,
                        name TEXT NOT NULL,
                        description TEXT NOT NULL,
                        style TEXT NOT NULL
                    );",
        )
        .await?;

        let admin_role_id = Uuid::new_v4().to_string();

        conn.execute(
            format!(
                "
            INSERT INTO groups (id, name, description)
            VALUES ('{}', 'admin', 'admin role');
        ",
                admin_role_id
            )
            .as_str(),
        )
        .await?;

        conn.execute(
            format!(
                "
            INSERT INTO groups (id, name, description)
            VALUES ('{}', 'operator', 'operator role');
        ",
                Uuid::new_v4().to_string()
            )
            .as_str(),
        )
        .await?;

        //create admin user with conn.execute
        let argon2 = Argon2::default();
        let salt = SaltString::encode_b64(salt.as_bytes()).unwrap();
        let password_hash = argon2
            .hash_password("admin".to_string().as_bytes(), &salt)
            .unwrap()
            .to_string();
        conn.execute(
            format!(
                "
            INSERT INTO users
                (id, username, email, password, groups)
            VALUES
                ('{}', 'admin', 'admin@gmail.com', '{password_hash}', '{admin_role_id}');",
                Uuid::new_v4().to_string(),
            )
            .as_str(),
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

    let db_conn = &format!("{}/mvtrs.db", app_config.config_dir);
    let salt = app_config.salt_string.clone();
    let cf_pool = init_sqlite(db_conn, salt).await?;

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

    let app_state = AppState {
        db_pool,
        cf_pool,
        catalog,
        disk_cache,
        auth,
        jwt_secret: app_config.jwt_secret,
        use_redis_cache,
        redis_cache,
        categories,
    };

    unsafe {
        APP_STATE.set(app_state).unwrap();
    }

    let acceptor = TcpListener::new(format!("{}:{}", app_config.host, app_config.port))
        .bind()
        .await;
    // dbg!(routes::app_router());
    Server::new(acceptor).serve(routes::app_router()).await;

    Ok(())
}
