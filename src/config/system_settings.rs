use sqlx::SqlitePool;

use crate::error::AppResult;

pub async fn get_plugins_version(pool: &SqlitePool) -> AppResult<i64> {
    let row: (String,) =
        sqlx::query_as("SELECT value FROM system_settings WHERE key = 'plugins_version'")
            .fetch_one(pool)
            .await?;
    Ok(row.0.parse().unwrap_or(0))
}

pub async fn bump_plugins_version(pool: &SqlitePool) -> AppResult<i64> {
    let new_version: (i64,) = sqlx::query_as(
        "UPDATE system_settings SET value = CAST(value AS INTEGER) + 1
         WHERE key = 'plugins_version'
         RETURNING CAST(value AS INTEGER)",
    )
    .fetch_one(pool)
    .await?;
    Ok(new_version.0)
}

pub async fn get_config_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (String,) =
        sqlx::query_as("SELECT value FROM system_settings WHERE key = 'config_version'")
            .fetch_one(pool)
            .await?;
    Ok(row.0.parse().unwrap_or(0))
}

pub async fn bump_config_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let new_version: (i64,) = sqlx::query_as(
        "UPDATE system_settings SET value = CAST(value AS INTEGER) + 1
         WHERE key = 'config_version'
         RETURNING CAST(value AS INTEGER)",
    )
    .fetch_one(pool)
    .await?;
    Ok(new_version.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn config_version_starts_at_zero() {
        let pool = in_memory_pool().await;
        assert_eq!(get_config_version(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn bump_increments_and_returns_new_value() {
        let pool = in_memory_pool().await;
        assert_eq!(bump_config_version(&pool).await.unwrap(), 1);
        assert_eq!(bump_config_version(&pool).await.unwrap(), 2);
        assert_eq!(get_config_version(&pool).await.unwrap(), 2);
    }
}
