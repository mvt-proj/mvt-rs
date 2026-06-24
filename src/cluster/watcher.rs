use std::time::Duration;
use tracing::{info, warn};

use crate::cluster::backend::SyncBackend;
use crate::cluster::snapshot::apply_snapshot;

/// Decides the next "known" version after a poll tick. The known version only
/// advances when a higher version was observed AND the reload succeeded, so a
/// failed reload is retried on the next tick.
pub fn next_known_version(known: i64, current: i64, reload_ok: bool) -> i64 {
    if current > known && reload_ok { current } else { known }
}

/// Spawns a background task that polls the config version every `interval` and,
/// when it rises, fetches a snapshot and swaps the in-memory state. A failed
/// reload is retried on the next tick (known version is not advanced).
pub fn start_config_watcher(backend: SyncBackend, interval: Duration, config_dir: String) {
    tokio::spawn(async move {
        let mut known = backend.current_version().await.unwrap_or(0);

        loop {
            tokio::time::sleep(interval).await;

            let current = match backend.current_version().await {
                Ok(v) => v,
                Err(e) => {
                    warn!("config watcher: failed to poll version: {e}");
                    continue;
                }
            };

            if current > known {
                let reload_ok = match backend.fetch_snapshot(&config_dir).await {
                    Ok(snapshot) => {
                        apply_snapshot(snapshot, &config_dir).await;
                        info!("config watcher: reloaded in-memory state ({known} -> {current})");
                        true
                    }
                    Err(e) => {
                        warn!("config watcher: reload failed, will retry next tick: {e}");
                        false
                    }
                };
                known = next_known_version(known, current, reload_ok);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_when_version_unchanged() {
        assert_eq!(next_known_version(5, 5, true), 5);
    }

    #[test]
    fn advances_when_higher_and_reload_ok() {
        assert_eq!(next_known_version(5, 6, true), 6);
    }

    #[test]
    fn stays_when_reload_failed() {
        assert_eq!(next_known_version(5, 6, false), 5);
    }
}
