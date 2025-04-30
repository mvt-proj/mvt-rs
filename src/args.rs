use crate::error::AppResult;
use clap::{Arg, Command};
use std::env;

#[derive(Debug)]
pub struct AppConfig {
    pub config_dir: String,
    pub cache_dir: String,
    pub map_assets_dir: String,
    pub host: String,
    pub port: String,
    pub db_conn: String,
    pub redis_conn: String,
    pub jwt_secret: String,
    pub session_secret: String,
    pub db_pool_size_min: u32,
    pub db_pool_size_max: u32,
}

pub async fn parse_args() -> AppResult<AppConfig> {
    dotenvy::dotenv().ok();

    let matches = Command::new("mvt-server: a vector tiles server")
        .arg(
            Arg::new("configdir")
                .short('c')
                .long("config")
                .value_name("CONFIGDIR")
                .help("Directory where config file is placed"),
        )
        .arg(
            Arg::new("cachedir")
                .short('b')
                .long("cache")
                .value_name("CACHEDIR")
                .help("Directory where cache files are placed"),
        )
        .arg(
            Arg::new("mapassetsdir")
                .short('m')
                .long("mapassets")
                .value_name("MAPASSETS")
                .help("Directory where map_assets files are placed"),
        )
        .arg(
            Arg::new("host")
                .short('i')
                .long("host")
                .value_name("HOST")
                .help("Bind address"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Bind port"),
        )
        .arg(
            Arg::new("dbconn")
                .short('d')
                .long("dbconn")
                .value_name("DBCONN")
                .help("Database connection"),
        )
        .arg(
            Arg::new("redisconn")
                .short('r')
                .long("redisconn")
                .value_name("REDISCONN")
                .help("Redis connection"),
        )
        .arg(
            Arg::new("jwtsecret")
                .short('j')
                .long("jwtsecret")
                .value_name("JWTSECRET")
                .help("JWT secret key"),
        )
        .arg(
            Arg::new("sessionsecret")
                .short('s')
                .long("sessionsecret")
                .value_name("SESSIONSECRET")
                .help("Session secret key"),
        )
        .arg(
            Arg::new("dbpoolmin")
                .short('n')
                .long("dbpoolmin")
                .value_name("DBPOOLMIN")
                .help("Minimum database pool size"),
        )
        .arg(
            Arg::new("dbpoolmax")
                .short('x')
                .long("dbpoolmax")
                .value_name("DBPOOLMAX")
                .help("Maximum database pool size"),
        )
        .get_matches();
    //
    // let get_value = |key: &str, arg_name: &str, default: Option<&str>| -> String {
    //     matches
    //         .get_one::<String>(arg_name)
    //         .cloned()
    //         .or_else(|| env::var(key).ok())
    //         .or_else(|| default.map(String::from))
    //         .unwrap_or_else(|| {
    //             panic!(
    //                 "Missing required config value for '{}'. Provide via CLI or env var.",
    //                 key
    //             )
    //         })
    // };

    let get_value = |key: &str, arg_name: &str, default: Option<&str>| -> String {
        matches
            .get_one::<String>(arg_name)
            .cloned()
            .or_else(|| env::var(key).ok())
            .or(default.map(String::from))
            .unwrap_or_else(|| {
                panic!(
                    "Missing required config value for '{}'. Provide via CLI, env var, or default.",
                    key
                )
            })
    };

    let config_dir = get_value("CONFIG", "configdir", Some("config"));
    let cache_dir = get_value("CACHE", "cachedir", Some("cache"));
    let map_assets_dir = get_value("MAPASSETS", "mapassetsdir", Some("map_assets"));
    let host = get_value("IPHOST", "host", Some("0.0.0.0"));
    let port = get_value("PORT", "port", Some("5800"));
    let db_conn = get_value("DBCONN", "dbconn", None);
    let redis_conn = get_value("REDISCONN", "redisconn", Some(""));
    let jwt_secret = get_value("JWTSECRET", "jwtsecret", None);
    let session_secret = get_value("SESSIONSECRET", "sessionsecret", None);

    let db_pool_size_min: u32 = get_value("POOLSIZEMIN", "dbpoolmin", Some("2"))
        .parse()
        .expect("Invalid POOLSIZEMIN value");
    let db_pool_size_max: u32 = get_value("POOLSIZEMAX", "dbpoolmax", Some("5"))
        .parse()
        .expect("Invalid POOLSIZEMAX value");

    Ok(AppConfig {
        config_dir,
        cache_dir,
        map_assets_dir,
        host,
        port,
        db_conn,
        redis_conn,
        jwt_secret,
        session_secret,
        db_pool_size_min,
        db_pool_size_max,
    })
}
