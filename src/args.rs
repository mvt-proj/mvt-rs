use crate::error::AppResult;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "mvt-server",
    about = "mvt-server: a vector tiles server",
    version,
    author
)]
pub struct AppConfig {
    #[arg(short = 'c', long = "config", env = "CONFIG", default_value = "config")]
    pub config_dir: String,

    #[arg(short = 'b', long = "cache", env = "CACHE", default_value = "cache")]
    pub cache_dir: String,

    #[arg(
        short = 'm',
        long = "mapassets",
        env = "MAPASSETS",
        default_value = "map_assets"
    )]
    pub map_assets_dir: String,

    #[arg(short = 'i', long = "host", env = "IPHOST", default_value = "0.0.0.0")]
    pub host: String,

    #[arg(short = 'p', long = "port", env = "PORT", default_value = "5800")]
    pub port: String,

    #[arg(short = 'd', long = "dbconn", env = "DBCONN")]
    pub db_conn: String,

    #[arg(short = 'r', long = "redisconn", env = "REDISCONN", default_value = "")]
    pub redis_conn: String,

    #[arg(short = 'j', long = "jwtsecret", env = "JWTSECRET")]
    pub jwt_secret: String,

    #[arg(short = 's', long = "sessionsecret", env = "SESSIONSECRET")]
    pub session_secret: String,

    #[arg(
        short = 'n',
        long = "dbpoolmin",
        env = "POOLSIZEMIN",
        default_value = "2"
    )]
    pub db_pool_size_min: u32,

    #[arg(
        short = 'x',
        long = "dbpoolmax",
        env = "POOLSIZEMAX",
        default_value = "5"
    )]
    pub db_pool_size_max: u32,

    #[arg(
        short = 'C',
        long = "config-cli",
        action = clap::ArgAction::SetTrue,
        help = "Enter to cli where you can set config values interactively"
    )]
    pub config_cli: bool,
}

pub async fn parse_args() -> AppResult<AppConfig> {
    dotenvy::dotenv().ok();
    let config = AppConfig::parse();

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_cli_overrides_env() {
        unsafe {
            env::set_var("PORT", "9000");
        }

        let args = vec![
            "mvt-server",
            "--port",
            "7000",
            "--dbconn",
            "db",
            "--jwtsecret",
            "js",
            "--sessionsecret",
            "ss",
        ];

        let config = AppConfig::parse_from(args);
        assert_eq!(config.port, "7000");

        unsafe {
            env::remove_var("PORT");
        }
    }

    #[test]
    fn test_env_var_loading() {
        unsafe {
            env::set_var("DBCONN", "env_db_connection");
            env::set_var("JWTSECRET", "env_secret");
            env::set_var("SESSIONSECRET", "env_session");
        }

        let args = vec!["mvt-server"];
        let config = AppConfig::parse_from(args);

        assert_eq!(config.db_conn, "env_db_connection");
        assert_eq!(config.jwt_secret, "env_secret");
        assert_eq!(config.session_secret, "env_session");

        unsafe {
            env::remove_var("DBCONN");
            env::remove_var("JWTSECRET");
            env::remove_var("SESSIONSECRET");
        }
    }
}
