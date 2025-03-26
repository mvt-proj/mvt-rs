use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;

use std::time::Duration;

use crate::error::AppResult;

pub async fn make_db_pool(
    db_conn: &str,
    min_connections: u32,
    max_connections: u32,
) -> AppResult<sqlx::Pool<sqlx::Postgres>> {
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
