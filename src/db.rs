use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;

use std::time::Duration;

pub async fn make_db_pool(
    db_conn: &str,
    min_connections: u32,
    max_connections: u32,
) -> anyhow::Result<sqlx::Pool<sqlx::Postgres>, anyhow::Error> {
    let mut opts: PgConnectOptions = db_conn.parse()?;

    opts = opts
        .log_statements(log::LevelFilter::Trace)
        .log_slow_statements(log::LevelFilter::Warn, Duration::from_secs(3));

    let pool = PgPoolOptions::new()
        .min_connections(min_connections)
        .max_connections(max_connections)
        .connect_with(opts)
        .await?;

    Ok(pool)
}
