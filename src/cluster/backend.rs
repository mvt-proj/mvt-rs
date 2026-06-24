use sqlx::SqlitePool;

use crate::cluster::snapshot::{ConfigSnapshot, build_snapshot};
use crate::config::system_settings::get_config_version;
use crate::error::AppResult;

/// Where an instance reads the config version and snapshot from.
pub enum SyncBackend {
    /// Reads from the local SQLite pool (shared-volume / owner).
    Local { pool: &'static SqlitePool },
    // Remote arm added in Task 11.
}

impl SyncBackend {
    pub async fn current_version(&self) -> AppResult<i64> {
        match self {
            SyncBackend::Local { pool } => Ok(get_config_version(pool).await?),
        }
    }

    pub async fn fetch_snapshot(&self, config_dir: &str) -> AppResult<ConfigSnapshot> {
        match self {
            SyncBackend::Local { pool } => build_snapshot(config_dir, pool).await,
        }
    }
}
