use std::time::Duration;
use tracing::{info, warn};

use crate::cluster::backend::SyncBackend;
use crate::cluster::snapshot::ConfigSnapshot;

/// Fetches the initial config snapshot from the owner, retrying every
/// `retry_interval` until it succeeds. A client cannot serve correct config
/// without it, so this intentionally blocks startup until the owner is reachable.
pub async fn bootstrap_from_owner(
    owner_url: &str,
    secret: &str,
    retry_interval: Duration,
) -> ConfigSnapshot {
    let backend = SyncBackend::remote(owner_url.to_string(), secret.to_string());
    loop {
        match backend.fetch_snapshot("").await {
            Ok(snapshot) => {
                info!("cluster client: fetched initial config snapshot from owner");
                return snapshot;
            }
            Err(e) => {
                warn!("cluster client: owner not ready ({e}); retrying in {retry_interval:?}");
                tokio::time::sleep(retry_interval).await;
            }
        }
    }
}
