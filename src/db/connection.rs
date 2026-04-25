use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgPool};
use sqlx::ConnectOptions;
use std::collections::HashMap;
use std::time::Duration;
use crate::error::{AppResult, AppError};

#[derive(Debug)]
pub struct DbRegistry {
    pools: HashMap<String, PgPool>,
}

impl DbRegistry {
    pub async fn new() -> AppResult<Self> {
        let mut pools = HashMap::new();

        for (key, value) in std::env::vars() {
            if key.starts_with("DBCONN") {
                let name = if key == "DBCONN" || key == "DBCONN_DEFAULT" {
                    "default".to_string()
                } else {
                    key.replace("DBCONN_", "").to_lowercase()
                };

                let pool = make_db_pool(&value, 1, 10).await?;
                pools.insert(name, pool);
            }
        }

        if pools.is_empty() {
            return Err(AppError::DatabaseError("No database connections configured.".to_string()));
        }

        Ok(Self { pools })
    }

    pub fn get_pool(&self, name: &str) -> Option<&PgPool> {
        self.pools.get(name)
    }

    pub fn get_default_pool(&self) -> &PgPool {
        self.pools.get("default").expect("Default pool must exist")
    }

    pub fn list_databases(&self) -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = self.pools.keys()
            .map(|s| {
                let label = {
                    let mut c = s.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                };
                (s.clone(), label)
            })
            .collect();
        keys.sort_by(|a, b| a.0.cmp(&b.0));
        keys
    }
}

pub async fn make_db_pool(
    db_conn: &str,
    min_connections: u32,
    max_connections: u32,
) -> AppResult<PgPool> {
    let mut opts: PgConnectOptions = db_conn.parse()?;

    opts = opts
        .log_statements(tracing::log::LevelFilter::Off)
        .log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_secs(3));

    let pool = PgPoolOptions::new()
        .min_connections(min_connections)
        .max_connections(max_connections)
        .connect_with(opts)
        .await?;

    Ok(pool)
}
