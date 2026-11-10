use crate::get_cf_pool;
use sqlx::{Row, sqlite::SqlitePool};
use std::collections::HashMap;

pub async fn get_plugins_enabled(
    pool: Option<&SqlitePool>,
) -> Result<HashMap<String, bool>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query("SELECT plugin_key, enabled FROM plugins_config")
        .fetch_all(pool)
        .await?;

    let mut map = HashMap::new();
    for row in rows {
        let key: String = row.get("plugin_key");
        let enabled: bool = row.get("enabled");
        map.insert(key, enabled);
    }

    Ok(map)
}

pub async fn set_plugin_enabled(
    pool: Option<&SqlitePool>,
    plugin_key: &str,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query(
        "INSERT INTO plugins_config (plugin_key, enabled) VALUES (?, ?)
         ON CONFLICT(plugin_key) DO UPDATE SET enabled = excluded.enabled",
    )
    .bind(plugin_key)
    .bind(enabled)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn get_plugins_enabled_returns_empty_map_when_no_rows() {
        let pool = in_memory_pool().await;
        let map = get_plugins_enabled(Some(&pool)).await.unwrap();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn set_plugin_enabled_persists_value() {
        let pool = in_memory_pool().await;
        set_plugin_enabled(Some(&pool), "mycat_mylayer", false)
            .await
            .unwrap();

        let map = get_plugins_enabled(Some(&pool)).await.unwrap();
        assert_eq!(map.get("mycat_mylayer"), Some(&false));
    }

    #[tokio::test]
    async fn set_plugin_enabled_twice_overwrites_previous_value() {
        let pool = in_memory_pool().await;
        set_plugin_enabled(Some(&pool), "mycat_mylayer", false)
            .await
            .unwrap();
        set_plugin_enabled(Some(&pool), "mycat_mylayer", true)
            .await
            .unwrap();

        let map = get_plugins_enabled(Some(&pool)).await.unwrap();
        assert_eq!(map.get("mycat_mylayer"), Some(&true));
    }
}
