use salvo::prelude::*;
use tracing::info;

use crate::{
    config::system_settings::bump_plugins_version,
    get_cf_pool, get_plugin_registry, get_plugins_dir,
    plugins::LuaPluginRegistry,
};

#[handler]
pub async fn reload(res: &mut Response) {
    let dir = get_plugins_dir();
    let new_registry = LuaPluginRegistry::new(dir);
    *get_plugin_registry().write().await = new_registry;

    match bump_plugins_version(get_cf_pool()).await {
        Ok(version) => {
            info!("Lua plugin registry reloaded from '{dir}' (version → {version})");
            res.render(Json(serde_json::json!({
                "status": "ok",
                "plugins_dir": dir,
                "plugins_version": version,
            })));
        }
        Err(e) => {
            info!("Lua plugin registry reloaded from '{dir}' (version bump failed: {e})");
            res.render(Json(serde_json::json!({
                "status": "ok",
                "plugins_dir": dir,
                "warning": format!("version bump failed: {e}"),
            })));
        }
    }
}
