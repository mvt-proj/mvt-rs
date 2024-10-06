use mvtrs::common::error::AppResult;
use clap::{Arg, Command};
// use std::path::Path;
// use tokio::fs::File;
// use tokio::io::AsyncWriteExt;

#[derive(Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: String,
    pub hostadmin: String,
    pub portadmin: String,
    pub db_conn: String,
    pub jwt_secret: String,
    pub salt_string: String,
}

pub async fn parse_args() -> AppResult<AppConfig> {
    let matches = Command::new("mvt-server: a vector tiles server")
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
            Arg::new("hostadmin")
                .short('f')
                .long("hostadmin")
                .value_name("HOST")
                .default_value("0.0.0.0")
                .help("Bind address"),
        )
        .arg(
            Arg::new("portadmin")
                .short('q')
                .long("portadmin")
                .value_name("PORT")
                .default_value("5888")
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
            Arg::new("jwtsecret")
                .short('j')
                .long("jwtsecret")
                .value_name("JWTSECRET")
                .required(false)
                .help("JWT secret key"),
        )
        .get_matches();


    dotenv::dotenv().ok();

    let mut host = String::new();
    let mut port = String::new();
    let mut hostadmin = String::new();
    let mut portadmin = String::new();
    let mut db_conn = String::new();
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

    if matches.contains_id("hostadmin") {
        hostadmin = matches
            .get_one::<String>("hostadmin")
            .expect("required")
            .to_string();
    }

    if matches.contains_id("portadmin") {
        portadmin = matches
            .get_one::<String>("portadmin")
            .expect("required")
            .to_string();
    }


    if matches.contains_id("dbconn") {
        db_conn = matches
            .get_one::<String>("dbconn")
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

    if hostadmin.is_empty() {
        hostadmin = std::env::var("IPHOSTADMIN").expect("IPHOSTADMIN needs to be defined");
    }

    if portadmin.is_empty() {
        portadmin = std::env::var("PORTADMIN").expect("PORTADMIN needs to be defined");
    }


    if db_conn.is_empty() {
        db_conn = std::env::var("DBCONN").expect("DBCONN needs to be defined");
    }

    if jwt_secret.is_empty() {
        jwt_secret = std::env::var("JWTSECRET").expect("JWTSECRET needs to be defined");
    }

    let salt_string = std::env::var("SALTSTRING").expect("SALTSTRING needs to be defined");


    Ok(AppConfig {
        host,
        port,
        hostadmin,
        portadmin,
        db_conn,
        jwt_secret,
        salt_string,
    })
}
