// use sqlx::SqlitePool;

use mvtrs::common::error::AppResult;

pub async fn make_db_pool(
    db_conn: &str,
) -> AppResult<sqlx::SqlitePool> {

    let pool = sqlx::SqlitePool::connect(db_conn).await?;
    Ok(pool)
}
