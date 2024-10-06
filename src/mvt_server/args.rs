use crate::error::{AppError, AppResult};
use clap::{Arg, Command};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[derive(Debug)]
pub struct AppConfig {
    pub config_dir: String,
    pub cache_dir: String,
    pub host: String,
    pub port: String,
    pub db_conn: String,
    pub redis_conn: String,
    pub jwt_secret: String,
    pub db_pool_size_min: u32,
    pub db_pool_size_max: u32,
    pub salt_string: String,
}

pub async fn parse_args() -> AppResult<AppConfig> {
    let matches = Command::new("mvt-server: a vector tiles server")
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

    let config_dir = matches
        .get_one::<String>("configdir")
        .expect("required")
        .to_string();

    let cache_dir = matches
        .get_one::<String>("cachedir")
        .expect("required")
        .to_string();

    create_config_files(&config_dir).await?;

    dotenv::dotenv().ok();

    let mut host = String::new();
    let mut port = String::new();
    let mut db_conn = String::new();
    let mut redis_conn = String::new();
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

    if redis_conn.is_empty() {
        redis_conn = std::env::var("REDISCONN").unwrap_or_default();
    }

    if jwt_secret.is_empty() {
        jwt_secret = std::env::var("JWTSECRET").expect("JWTSECRET needs to be defined");
    }

    let db_pool_size_min = std::env::var("POOLSIZEMIN").unwrap_or("2".to_string());
    let db_pool_size_max = std::env::var("POOLSIZEMAX").unwrap_or("5".to_string());
    let salt_string = std::env::var("SALTSTRING").expect("SALTSTRING needs to be defined");

    let db_pool_size_min: u32 = db_pool_size_min.parse().unwrap_or(3);
    let db_pool_size_max: u32 = db_pool_size_max.parse().unwrap_or(5);

    Ok(AppConfig {
        config_dir,
        cache_dir,
        host,
        port,
        db_conn,
        redis_conn,
        jwt_secret,
        db_pool_size_min,
        db_pool_size_max,
        salt_string,
    })
}

async fn create_config_files(config_dir: &str) -> AppResult<()> {
    let dir_path = Path::new(config_dir);
    if !dir_path.exists() {
        tokio::fs::create_dir_all(dir_path)
            .await
            .map_err(AppError::FileCreationError)?;
    }

    let paths_to_create = ["catalog.json", "users.json", "groups.json"];
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
