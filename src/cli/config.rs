use crate::args::AppConfig;
use std::fs;
use std::io;
use std::path::Path;

pub fn save_config_to_env(config: &AppConfig, path: &str) -> io::Result<()> {
    let mut lines = Vec::new();

    if !config.config_dir.is_empty() {
        lines.push(format!("CONFIG={}", config.config_dir));
    }
    if !config.cache_dir.is_empty() {
        lines.push(format!("CACHE={}", config.cache_dir));
    }
    if !config.map_assets_dir.is_empty() {
        lines.push(format!("MAPASSETS={}", config.map_assets_dir));
    }
    if !config.host.is_empty() {
        lines.push(format!("IPHOST={}", config.host));
    }
    if !config.port.is_empty() {
        lines.push(format!("PORT={}", config.port));
    }
    if !config.db_conn.is_empty() {
        lines.push(format!("DBCONN={}", config.db_conn));
    }
    if !config.redis_conn.is_empty() {
        lines.push(format!("REDISCONN={}", config.redis_conn));
    }
    if !config.jwt_secret.is_empty() {
        lines.push(format!("JWTSECRET={}", config.jwt_secret));
    }
    if !config.session_secret.is_empty() {
        lines.push(format!("SESSIONSECRET={}", config.session_secret));
    }

    lines.push(format!("POOLSIZEMIN={}", config.db_pool_size_min));
    lines.push(format!("POOLSIZEMAX={}", config.db_pool_size_max));

    fs::write(path, lines.join("\n") + "\n")
}

pub fn env_exists(path: &str) -> bool {
    Path::new(path).exists()
}
