use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub sqlite_path: String,
    pub redis_url: Option<String>,
    pub pool_min: u32,
    pub pool_max: u32,
}

#[derive(Debug, Deserialize)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub session_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct PathConfig {
    pub config: String,
    pub cache: String,
    pub assets: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub postgres_databases: HashMap<String, String>,
    pub security: SecurityConfig,
    pub paths: PathConfig,
}

impl Settings {
    pub fn new(config_path: &str) -> Result<Self, config::ConfigError> {
        let s = config::Config::builder()
            .add_source(config::File::with_name(config_path).required(false))
            .add_source(config::Environment::with_prefix("MVT"))
            .add_source(config::Environment::default())
            .build()
            .map_err(|e| {
                eprintln!("Error loading configuration: {}", e);
                e
            })?;

        s.try_deserialize().map_err(|e| {
            eprintln!("Error deserializing configuration: {}. Ensure all required fields (database, security, paths) are set in YAML or environment variables.", e);
            e
        })
    }
}
