use serde::Deserialize;
use sqlx::SqlitePool;

use crate::cluster::snapshot::{ConfigSnapshot, build_snapshot};
use crate::config::system_settings::get_config_version;
use crate::error::AppResult;

#[derive(Deserialize)]
struct VersionResponse {
    version: i64,
}

/// Where an instance reads the config version and snapshot from.
pub enum SyncBackend {
    /// Reads from the local SQLite pool (shared-volume / owner).
    Local { pool: &'static SqlitePool },
    /// Reads from a remote owner instance over HTTP.
    Remote {
        owner_url: String,
        secret: String,
        client: reqwest::Client,
    },
}

impl SyncBackend {
    pub fn remote(owner_url: String, secret: String) -> Self {
        SyncBackend::Remote {
            owner_url: owner_url.trim_end_matches('/').to_string(),
            secret,
            client: reqwest::Client::new(),
        }
    }

    pub async fn current_version(&self) -> AppResult<i64> {
        match self {
            SyncBackend::Local { pool } => Ok(get_config_version(pool).await?),
            SyncBackend::Remote { owner_url, secret, client } => {
                let resp: VersionResponse = client
                    .get(format!("{owner_url}/internal/config/version"))
                    .header("x-cluster-secret", secret)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                Ok(resp.version)
            }
        }
    }

    pub async fn fetch_snapshot(&self, config_dir: &str) -> AppResult<ConfigSnapshot> {
        match self {
            SyncBackend::Local { pool } => build_snapshot(config_dir, pool).await,
            SyncBackend::Remote { owner_url, secret, client } => {
                let snapshot: ConfigSnapshot = client
                    .get(format!("{owner_url}/internal/config/snapshot"))
                    .header("x-cluster-secret", secret)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                Ok(snapshot)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn remote_reads_version_from_owner_api() {
        // This test documents the Remote contract. It requires the owner's
        // internal router and a reachable HTTP endpoint; it is wired in Step 5.
        let backend = SyncBackend::remote(
            "http://127.0.0.1:5899".to_string(),
            "test-secret".to_string(),
        );
        // With no server running, current_version must return an Err (not panic).
        assert!(backend.current_version().await.is_err());
    }
}
