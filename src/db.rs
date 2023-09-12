use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;

pub async fn make_db_pool(
    db_url: &str,
    min_connections: u32,
    max_connections: u32,
) -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    let connection_options = PgConnectOptions::from_str(db_url).unwrap();
    connection_options.clone().disable_statement_logging();

    let pool = match PgPoolOptions::new()
        .min_connections(min_connections)
        .max_connections(max_connections)
        .connect_with(connection_options)
        .await
    {
        Ok(pool) => pool,
        Err(e) => return Err(e),
    };
    Ok(pool)
}
