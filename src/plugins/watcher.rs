use std::time::Duration;
use tracing::{info, warn};

use crate::{
    config::system_settings::get_plugins_version,
    get_cf_pool, get_plugin_registry, get_plugins_dir,
    plugins::LuaPluginRegistry,
};

const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Spawns a background task that polls `system_settings.plugins_version` every
/// 30 seconds. When the version in the shared SQLite DB is higher than the
/// locally known one, all instances reload their plugin registry automatically.
pub fn start_plugin_watcher() {
    tokio::spawn(async move {
        let pool = get_cf_pool();
        let mut known_version = match get_plugins_version(pool).await {
            Ok(v) => v,
            Err(e) => {
                warn!("Plugin watcher: failed to read initial version: {e}");
                0
            }
        };

        loop {
            tokio::time::sleep(POLL_INTERVAL).await;

            let current = match get_plugins_version(pool).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("Plugin watcher: failed to poll version: {e}");
                    continue;
                }
            };

            if current > known_version {
                let dir = get_plugins_dir();
                let new_registry = LuaPluginRegistry::new(dir);
                *get_plugin_registry().write().await = new_registry;
                info!("Plugin watcher: reloaded plugins from '{dir}' (version {known_version} → {current})");
                known_version = current;
            }
        }
    });
}
