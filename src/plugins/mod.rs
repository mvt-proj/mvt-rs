use mlua::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Context passed to every Lua hook.
pub struct PluginContext {
    pub layer: String,
    pub category: String,
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

/// Holds one Lua VM per plugin file.
///
/// File naming convention:
///   `{category}.lua`            → category-level plugin (applies to all layers in the category)
///   `{category}_{layer}.lua`    → layer-level plugin (applies to one specific layer)
///
/// When both exist for a given request, both `filter()` functions run and
/// their WHERE clauses are combined with AND.
pub struct LuaPluginRegistry {
    plugins: HashMap<String, Mutex<Lua>>,
}

impl std::fmt::Debug for LuaPluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaPluginRegistry")
            .field("plugins", &self.plugins.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl LuaPluginRegistry {
    /// Scans `plugins_dir` at startup and loads every `.lua` file found.
    /// Missing or unreadable directory is silently ignored (no plugins active).
    pub fn new(plugins_dir: &str) -> Self {
        let mut plugins = HashMap::new();
        let dir = Path::new(plugins_dir);

        if !dir.exists() {
            info!("Plugins directory '{}' not found — no plugins loaded", plugins_dir);
            return Self { plugins };
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("Cannot read plugins directory '{}': {}", plugins_dir, e);
                return Self { plugins };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }

            let key = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            let script = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Cannot read plugin {:?}: {}", path, e);
                    continue;
                }
            };

            match Self::load_plugin(&key, &script) {
                Ok(lua) => {
                    info!("Loaded Lua plugin: '{}' ({:?})", key, path);
                    plugins.insert(key, Mutex::new(lua));
                }
                Err(e) => {
                    warn!("Failed to load plugin '{}': {}", key, e);
                }
            }
        }

        Self { plugins }
    }

    fn load_plugin(key: &str, script: &str) -> LuaResult<Lua> {
        let lua = Lua::new();

        // Expose log(msg) so scripts can write to the server's tracing log.
        let k = key.to_string();
        let log_fn = lua.create_function(move |_, msg: String| {
            info!(target: "mvt_server::plugins", plugin = %k, "{}", msg);
            Ok(())
        })?;
        lua.globals().set("log", log_fn)?;

        lua.load(script).exec()?;
        Ok(lua)
    }

    /// Returns true if a category-level or layer-level plugin exists for this layer.
    pub fn has_plugin(&self, layer_key: &str, category: &str) -> bool {
        self.plugins.contains_key(layer_key) || self.plugins.contains_key(category)
    }

    /// Calls `filter(ctx)` on the category plugin and/or the layer plugin.
    /// Returns the combined WHERE clause (ANDed), or `None` if no plugin exists.
    pub async fn call_filter(
        &self,
        layer_key: &str,
        category: &str,
        ctx: &PluginContext,
    ) -> Option<String> {
        let cat_result = self.call_single_filter(category, ctx).await;
        let layer_result = self.call_single_filter(layer_key, ctx).await;

        // None + None → no plugin active
        if cat_result.is_none() && layer_result.is_none() {
            return None;
        }

        let mut clauses: Vec<String> = Vec::new();
        if let Some(s) = cat_result {
            if !s.is_empty() {
                clauses.push(s);
            }
        }
        if let Some(s) = layer_result {
            if !s.is_empty() {
                clauses.push(s);
            }
        }

        Some(if clauses.is_empty() {
            String::new()
        } else {
            clauses.join(" AND ")
        })
    }

    async fn call_single_filter(&self, key: &str, ctx: &PluginContext) -> Option<String> {
        let lua_mutex = self.plugins.get(key)?;
        let lua = lua_mutex.lock().await;

        let result = (|| -> LuaResult<String> {
            let filter_fn: LuaFunction = lua.globals().get("filter")?;
            let lua_ctx = lua.create_table()?;
            lua_ctx.set("layer", ctx.layer.as_str())?;
            lua_ctx.set("category", ctx.category.as_str())?;
            lua_ctx.set("z", ctx.z)?;
            lua_ctx.set("x", ctx.x)?;
            lua_ctx.set("y", ctx.y)?;
            filter_fn.call(lua_ctx)
        })();

        match result {
            Ok(s) => Some(s),
            Err(e) => {
                warn!(plugin = %key, error = %e, "Lua filter hook error");
                None
            }
        }
    }
}
