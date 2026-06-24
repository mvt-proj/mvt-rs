use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct CliArgs {
    #[arg(short, long)]
    pub config: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 5887 }

#[derive(Debug, Deserialize, Default)]
pub struct ClusterConfig {
    #[serde(default = "default_cluster_mode")]
    pub mode: String,
    #[serde(default = "default_config_watch_interval")]
    pub config_watch_interval_secs: u64,
    pub owner_url: Option<String>,
    pub shared_secret: Option<String>,
}

fn default_cluster_mode() -> String { "standalone".to_string() }
fn default_config_watch_interval() -> u64 { 10 }

#[derive(Debug, Deserialize, Default)]
pub struct DatabaseConfig {
    #[serde(default = "default_sqlite")]
    pub sqlite_path: String,
    pub redis_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PostgresDatabasesConfig {
    #[serde(default = "default_pool_min")]
    pub pool_min: u32,
    #[serde(default = "default_pool_max")]
    pub pool_max: u32,
    #[serde(flatten)]
    pub connections: HashMap<String, String>,
}

fn default_sqlite() -> String { "mvtrs.db".to_string() }
fn default_pool_min() -> u32 { 2 }
fn default_pool_max() -> u32 { 5 }

fn default_session_duration() -> u64 { 20 }

#[derive(Debug, Deserialize, Default)]
pub struct SecurityConfig {
    #[serde(default)] pub jwt_secret: String,
    #[serde(default)] pub session_secret: String,
    #[serde(default = "default_session_duration")] pub session_duration_minutes: u64,
}

#[derive(Debug, Deserialize, Default)]
pub struct PathConfig {
    #[serde(default = "default_config_path")]  pub config: String,
    #[serde(default = "default_cache_path")]   pub cache: String,
    #[serde(default = "default_assets_path")]  pub assets: String,
    #[serde(default = "default_plugins_path")] pub plugins: String,
}

fn default_config_path() -> String { "config".to_string() }
fn default_cache_path() -> String { "cache".to_string() }
fn default_assets_path() -> String { "map_assets".to_string() }
fn default_plugins_path() -> String { "plugins".to_string() }

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    #[serde(default)] pub server: ServerConfig,
    #[serde(default)] pub database: DatabaseConfig,
    #[serde(default)] pub postgres_databases: PostgresDatabasesConfig,
    #[serde(default)] pub security: SecurityConfig,
    #[serde(default)] pub paths: PathConfig,
    #[serde(default)] pub cluster: ClusterConfig,
}

impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        let args = CliArgs::parse();
        let config_path = args
            .config
            .unwrap_or_else(|| "config/config.yaml".to_string());

        let mut builder = config::Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 5887)?
            .set_default("database.sqlite_path", "mvtrs.db")?
            .set_default("postgres_databases.pool_min", 2)?
            .set_default("postgres_databases.pool_max", 5)?
            .set_default("paths.config", "config")?
            .set_default("paths.cache", "cache")?
            .set_default("paths.assets", "map_assets")?
            .set_default("paths.plugins", "plugins")?
            .set_default("cluster.mode", "standalone")?
            .set_default("cluster.config_watch_interval_secs", 10)?
            .add_source(
                config::File::new(&config_path, config::FileFormat::Yaml).required(false),
            )
            .add_source(
                config::Environment::with_prefix("MVT")
                    .prefix_separator("_")
                    .separator("__")
                    .try_parsing(true),
            );

        // CLI overrides — máxima prioridad
        if let Some(host) = args.host {
            builder = builder.set_override("server.host", host)?;
        }
        if let Some(port) = args.port {
            builder = builder.set_override("server.port", port)?;
        }

        let s = builder.build().map_err(|e| {
            tracing::error!("Error loading configuration: {}", e);
            e
        })?;

        let settings: Settings = s.try_deserialize().map_err(|e| {
            tracing::error!("Error deserializing configuration: {}", e);
            e
        })?;

        tracing::debug!("Loaded settings: {:?}", settings);

        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.postgres_databases.connections.contains_key("default") {
            return Err(
                "Configuration error: 'postgres_databases' must contain a 'default' entry. \
                Add it to config.yaml under 'postgres_databases.default' or set \
                MVT_POSTGRES_DATABASES__DEFAULT env var."
                    .to_string(),
            );
        }

        if self.security.jwt_secret.len() < 32 {
            return Err(format!(
                "Configuration error: 'security.jwt_secret' must be at least 32 characters \
                (current: {}). Set it in config.yaml or via MVT_SECURITY__JWT_SECRET.",
                self.security.jwt_secret.len()
            ));
        }

        if self.server.port == 0 {
            return Err(
                "Configuration error: 'server.port' must be between 1 and 65535.".to_string(),
            );
        }

        if self.database.redis_url.as_deref().is_some_and(|url| url.is_empty()) {
            return Err(
                "Configuration error: 'database.redis_url' is set but empty. \
                Either provide a valid Redis URL or remove the key entirely."
                    .to_string(),
            );
        }

        match self.cluster.mode.as_str() {
            "standalone" | "shared" => {}
            "owner" => {
                if self.cluster.shared_secret.as_deref().unwrap_or("").is_empty() {
                    return Err("Configuration error: cluster.mode = owner requires \
                        cluster.shared_secret".to_string());
                }
            }
            "client" => {
                if self.cluster.owner_url.as_deref().unwrap_or("").is_empty() {
                    return Err("Configuration error: cluster.mode = client requires \
                        cluster.owner_url".to_string());
                }
                if self.cluster.shared_secret.as_deref().unwrap_or("").is_empty() {
                    return Err("Configuration error: cluster.mode = client requires \
                        cluster.shared_secret".to_string());
                }
            }
            other => {
                return Err(format!(
                    "Configuration error: invalid cluster.mode '{other}' \
                     (expected standalone | shared | owner | client)"
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_settings() -> Settings {
        let mut connections = std::collections::HashMap::new();
        connections.insert(
            "default".to_string(),
            "postgres://user:pass@localhost/db".to_string(),
        );
        Settings {
            server: ServerConfig { host: "0.0.0.0".to_string(), port: 5887 },
            database: DatabaseConfig {
                sqlite_path: "mvtrs.db".to_string(),
                redis_url: None,
            },
            postgres_databases: PostgresDatabasesConfig {
                pool_min: 2,
                pool_max: 5,
                connections,
            },
            security: SecurityConfig {
                jwt_secret: "a-secret-that-is-at-least-32-chars-long-here".to_string(),
                session_secret: "a-session-secret-at-least-32-chars-long-here".to_string(),
                session_duration_minutes: 20,
            },
            paths: PathConfig {
                config: "config".to_string(),
                cache: "cache".to_string(),
                assets: "map_assets".to_string(),
                plugins: "plugins".to_string(),
            },
            cluster: ClusterConfig {
                mode: "standalone".to_string(),
                config_watch_interval_secs: 10,
                owner_url: None,
                shared_secret: None,
            },
        }
    }

    #[test]
    fn valid_settings_passes() {
        assert!(valid_settings().validate().is_ok());
    }

    #[test]
    fn missing_default_db_fails() {
        let mut s = valid_settings();
        s.postgres_databases.connections.clear();
        let err = s.validate().unwrap_err();
        assert!(err.contains("default"));
    }

    #[test]
    fn short_jwt_secret_fails() {
        let mut s = valid_settings();
        s.security.jwt_secret = "short".to_string();
        let err = s.validate().unwrap_err();
        assert!(err.contains("jwt_secret"));
    }

    #[test]
    fn zero_port_fails() {
        let mut s = valid_settings();
        s.server.port = 0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn empty_redis_url_fails() {
        let mut s = valid_settings();
        s.database.redis_url = Some("".to_string());
        let err = s.validate().unwrap_err();
        assert!(err.contains("redis_url"));
    }

    #[test]
    fn cluster_mode_default_is_standalone() {
        assert_eq!(default_cluster_mode(), "standalone");
    }

    #[test]
    fn config_watch_interval_default_is_ten() {
        assert_eq!(default_config_watch_interval(), 10);
    }

    #[test]
    fn invalid_cluster_mode_fails() {
        let mut s = valid_settings();
        s.cluster.mode = "bogus".to_string();
        assert!(s.validate().is_err());
    }

    #[test]
    fn client_requires_owner_url_and_secret() {
        let mut s = valid_settings();
        s.cluster.mode = "client".to_string();
        assert!(s.validate().is_err());
        s.cluster.owner_url = Some("https://owner:5887".to_string());
        // owner_url set but shared_secret still missing → should fail
        assert!(s.validate().is_err());
        s.cluster.shared_secret = Some("a-cluster-secret-value".to_string());
        assert!(s.validate().is_ok());
    }

    #[test]
    fn owner_requires_secret() {
        let mut s = valid_settings();
        s.cluster.mode = "owner".to_string();
        assert!(s.validate().is_err());
        s.cluster.shared_secret = Some("a-cluster-secret-value".to_string());
        assert!(s.validate().is_ok());
    }
}
