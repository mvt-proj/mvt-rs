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
        let path = std::path::Path::new(config_path);
        
        eprintln!("DEBUG: Loading config from: {:?}", path.canonicalize());

        let s = config::Config::builder()
            .add_source(config::File::from(path).format(config::FileFormat::Yaml).required(true))
            .add_source(config::Environment::with_prefix("MVT").separator("__"))
            .add_source(config::Environment::default())
            .build()
            .map_err(|e| {
                eprintln!("CRITICAL: Failed to load config file: {}", e);
                e
            })?;

        s.try_deserialize().map_err(|e| {
            eprintln!("CRITICAL: Failed to parse YAML: {}. Check structure.", e);
            e
        })
    }
}

