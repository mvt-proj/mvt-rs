use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::auth::Auth;
use crate::config::categories::get_categories as get_cf_categories;
use crate::config::styles::get_styles;
use crate::error::AppResult;
use crate::models::{catalog::Catalog, category::Category, styles::Style};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub catalog: Catalog,
    pub categories: Vec<Category>,
    pub auth: Auth,
    pub styles: Vec<Style>,
}

/// Builds a full config snapshot from the SQLite config DB.
pub async fn build_snapshot(config_dir: &str, pool: &SqlitePool) -> AppResult<ConfigSnapshot> {
    Ok(ConfigSnapshot {
        catalog: Catalog::new(pool).await?,
        categories: get_cf_categories(Some(pool)).await?,
        auth: Auth::new(config_dir, pool).await?,
        styles: get_styles(Some(pool)).await?,
    })
}

/// Swaps the four in-memory states under their RwLocks. `config_dir` is the
/// local instance's value and overrides whatever the snapshot's Auth carried,
/// so a client does not inherit the owner's paths.
pub async fn apply_snapshot(snapshot: ConfigSnapshot, config_dir: &str) {
    let ConfigSnapshot { catalog, categories, mut auth, styles } = snapshot;
    auth.config_dir = config_dir.to_string();

    *crate::get_catalog().await.write().await = catalog;
    *crate::get_categories().await.write().await = categories;
    *crate::get_auth().await.write().await = auth;
    *crate::get_styles_cache().await.write().await = styles;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn snapshot_round_trips_through_json() {
        let pool = in_memory_pool().await;
        let snap = build_snapshot("config", &pool).await.unwrap();
        let json = serde_json::to_string(&snap).unwrap();
        let back: ConfigSnapshot = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }
}
