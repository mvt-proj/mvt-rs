use mlua::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Parsed `-- @key value` annotations from the top of a Lua plugin file.
#[derive(Debug, Clone, Default)]
pub struct PluginDoc {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
}

/// Public metadata about a loaded plugin, used by the admin UI.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub key: String,
    pub source: String,
    pub doc: PluginDoc,
}

fn parse_doc(source: &str) -> PluginDoc {
    let mut doc = PluginDoc::default();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("--") {
            break;
        }
        let content = trimmed.trim_start_matches('-').trim();
        if let Some(v) = content.strip_prefix("@name") {
            doc.name = Some(v.trim().to_string());
        } else if let Some(v) = content.strip_prefix("@description") {
            doc.description = Some(v.trim().to_string());
        } else if let Some(v) = content.strip_prefix("@author") {
            doc.author = Some(v.trim().to_string());
        } else if let Some(v) = content.strip_prefix("@version") {
            doc.version = Some(v.trim().to_string());
        }
    }
    doc
}

/// Context passed to every Lua hook.
pub struct PluginContext {
    pub layer: String,
    pub category: String,
    pub z: u32,
    pub x: u32,
    pub y: u32,
    pub user: Option<String>,
    pub groups: Option<Vec<String>>,
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
    info: Vec<PluginInfo>,
}

impl std::fmt::Debug for LuaPluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaPluginRegistry")
            .field("plugins", &self.plugins.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl LuaPluginRegistry {
    pub fn list_plugins(&self) -> &[PluginInfo] {
        &self.info
    }
}

impl LuaPluginRegistry {
    /// Scans `plugins_dir` at startup and loads every `.lua` file found.
    /// Missing or unreadable directory is silently ignored (no plugins active).
    pub fn new(plugins_dir: &str) -> Self {
        let mut plugins = HashMap::new();
        let mut plugin_list: Vec<PluginInfo> = Vec::new();
        let dir = Path::new(plugins_dir);

        if !dir.exists() {
            info!("Plugins directory '{}' not found — no plugins loaded", plugins_dir);
            return Self { plugins, info: plugin_list };
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("Cannot read plugins directory '{}': {}", plugins_dir, e);
                return Self { plugins, info: plugin_list };
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

            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Cannot read plugin {:?}: {}", path, e);
                    continue;
                }
            };

            match Self::load_plugin(&key, &source) {
                Ok(lua) => {
                    info!("Loaded Lua plugin: '{}' ({:?})", key, path);
                    let doc = parse_doc(&source);
                    plugin_list.push(PluginInfo { key: key.clone(), source, doc });
                    plugins.insert(key, Mutex::new(lua));
                }
                Err(e) => {
                    warn!("Failed to load plugin '{}': {}", key, e);
                }
            }
        }

        plugin_list.sort_by(|a, b| a.key.cmp(&b.key));
        Self { plugins, info: plugin_list }
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
            lua_ctx.set("user", ctx.user.as_deref())?;
            let lua_groups = lua.create_table()?;
            for (i, g) in ctx.groups.iter().flatten().enumerate() {
                lua_groups.set(i + 1, g.as_str())?;
            }
            lua_ctx.set("groups", lua_groups)?;
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

    /// Builds a registry directly from a map of key → script source.
    /// Only used in tests to avoid filesystem dependency.
    #[cfg(test)]
    fn from_scripts(scripts: &[(&str, &str)]) -> Self {
        let mut plugins = HashMap::new();
        for (key, script) in scripts {
            match Self::load_plugin(key, script) {
                Ok(lua) => {
                    plugins.insert(key.to_string(), Mutex::new(lua));
                }
                Err(e) => panic!("Test plugin '{}' failed to load: {}", key, e),
            }
        }
        Self { plugins, info: Vec::new() }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(layer: &str, category: &str, z: u32) -> PluginContext {
        PluginContext {
            layer: layer.to_string(),
            category: category.to_string(),
            z,
            x: 0,
            y: 0,
            user: None,
            groups: None,
        }
    }

    // --- has_plugin ----------------------------------------------------------

    #[test]
    fn has_plugin_returns_false_when_no_plugin_exists() {
        let registry = LuaPluginRegistry::from_scripts(&[]);
        assert!(!registry.has_plugin("mycat_mylayer", "mycat"));
    }

    #[test]
    fn has_plugin_returns_true_for_layer_plugin() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat_mylayer",
            "function filter(ctx) return '' end",
        )]);
        assert!(registry.has_plugin("mycat_mylayer", "mycat"));
    }

    #[test]
    fn has_plugin_returns_true_for_category_plugin() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat",
            "function filter(ctx) return '' end",
        )]);
        assert!(registry.has_plugin("mycat_mylayer", "mycat"));
    }

    // --- call_filter: no plugin ----------------------------------------------

    #[tokio::test]
    async fn call_filter_returns_none_when_no_plugin() {
        let registry = LuaPluginRegistry::from_scripts(&[]);
        let result = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10)).await;
        assert!(result.is_none());
    }

    // --- call_filter: layer plugin only --------------------------------------

    #[tokio::test]
    async fn layer_plugin_returns_empty_string() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat_mylayer",
            "function filter(ctx) return '' end",
        )]);
        let result = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10)).await;
        assert_eq!(result, Some(String::new()));
    }

    #[tokio::test]
    async fn layer_plugin_returns_where_clause() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat_mylayer",
            "function filter(ctx) return \"population > 1000\" end",
        )]);
        let result = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10)).await;
        assert_eq!(result, Some("population > 1000".to_string()));
    }

    #[tokio::test]
    async fn layer_plugin_uses_zoom_in_logic() {
        let script = r#"
            function filter(ctx)
                if ctx.z < 10 then
                    return "area > 5000"
                end
                return ""
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("mycat_mylayer", script)]);

        let low_zoom = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 8)).await;
        assert_eq!(low_zoom, Some("area > 5000".to_string()));

        let high_zoom = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 14)).await;
        assert_eq!(high_zoom, Some(String::new()));
    }

    #[tokio::test]
    async fn layer_plugin_receives_correct_context_fields() {
        let script = r#"
            function filter(ctx)
                return ctx.category .. "_" .. ctx.layer .. "_" .. ctx.z
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("pub_roads", script)]);
        let result = registry.call_filter("pub_roads", "pub", &ctx("roads", "pub", 12)).await;
        assert_eq!(result, Some("pub_roads_12".to_string()));
    }

    // --- call_filter: category plugin only -----------------------------------

    #[tokio::test]
    async fn category_plugin_applies_to_any_layer_in_category() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat",
            "function filter(ctx) return \"active = true\" end",
        )]);

        let r1 = registry.call_filter("mycat_layer1", "mycat", &ctx("layer1", "mycat", 10)).await;
        let r2 = registry.call_filter("mycat_layer2", "mycat", &ctx("layer2", "mycat", 10)).await;

        assert_eq!(r1, Some("active = true".to_string()));
        assert_eq!(r2, Some("active = true".to_string()));
    }

    #[tokio::test]
    async fn category_plugin_does_not_apply_to_other_category() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat",
            "function filter(ctx) return \"active = true\" end",
        )]);
        let result = registry.call_filter("othercat_layer", "othercat", &ctx("layer", "othercat", 10)).await;
        assert!(result.is_none());
    }

    // --- call_filter: both plugins -------------------------------------------

    #[tokio::test]
    async fn category_and_layer_clauses_are_combined_with_and() {
        let registry = LuaPluginRegistry::from_scripts(&[
            ("mycat",        "function filter(ctx) return \"active = true\" end"),
            ("mycat_roads",  "function filter(ctx) return \"type = 'primary'\" end"),
        ]);
        let result = registry.call_filter("mycat_roads", "mycat", &ctx("roads", "mycat", 10)).await;
        assert_eq!(result, Some("active = true AND type = 'primary'".to_string()));
    }

    #[tokio::test]
    async fn combined_result_skips_empty_clauses() {
        let registry = LuaPluginRegistry::from_scripts(&[
            ("mycat",       "function filter(ctx) return \"\" end"),
            ("mycat_roads", "function filter(ctx) return \"type = 'primary'\" end"),
        ]);
        let result = registry.call_filter("mycat_roads", "mycat", &ctx("roads", "mycat", 10)).await;
        // category returns "", layer returns clause → only the clause, no leading AND
        assert_eq!(result, Some("type = 'primary'".to_string()));
    }

    #[tokio::test]
    async fn both_plugins_return_empty_gives_some_empty_string() {
        // Some("") != None: plugin exists (cache bypass) but no WHERE added
        let registry = LuaPluginRegistry::from_scripts(&[
            ("mycat",       "function filter(ctx) return \"\" end"),
            ("mycat_roads", "function filter(ctx) return \"\" end"),
        ]);
        let result = registry.call_filter("mycat_roads", "mycat", &ctx("roads", "mycat", 10)).await;
        assert_eq!(result, Some(String::new()));
    }

    // --- time / blocking filter ----------------------------------------------

    #[tokio::test]
    async fn filter_can_return_always_false_condition() {
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat_mylayer",
            "function filter(ctx) return \"1=0\" end",
        )]);
        let result = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10)).await;
        assert_eq!(result, Some("1=0".to_string()));
    }

    // --- error handling ------------------------------------------------------

    #[test]
    fn invalid_lua_syntax_is_rejected_at_load() {
        let result = LuaPluginRegistry::load_plugin("bad", "this is not lua @@@@");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn runtime_error_in_filter_returns_none_for_that_plugin() {
        // Script loads fine but filter() raises a runtime error
        let script = r#"
            function filter(ctx)
                error("something went wrong")
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("mycat_mylayer", script)]);
        // Runtime error → None (logged as warning, does not crash the request)
        let result = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10)).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn missing_filter_function_returns_none() {
        // Script loads but defines no filter() function
        let registry = LuaPluginRegistry::from_scripts(&[(
            "mycat_mylayer",
            "-- no filter function defined",
        )]);
        let result = registry.call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10)).await;
        assert!(result.is_none());
    }

    // --- ctx.user / ctx.groups -----------------------------------------------

    fn ctx_with_user(layer: &str, category: &str, user: &str, groups: &[&str]) -> PluginContext {
        PluginContext {
            layer: layer.to_string(),
            category: category.to_string(),
            z: 10,
            x: 0,
            y: 0,
            user: Some(user.to_string()),
            groups: Some(groups.iter().map(|s| s.to_string()).collect()),
        }
    }

    #[tokio::test]
    async fn filter_receives_user_as_string() {
        let script = r#"
            function filter(ctx)
                return ctx.user
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("mycat_mylayer", script)]);
        let result = registry
            .call_filter("mycat_mylayer", "mycat", &ctx_with_user("mylayer", "mycat", "alice", &[]))
            .await;
        assert_eq!(result, Some("alice".to_string()));
    }

    #[tokio::test]
    async fn filter_receives_nil_user_when_unauthenticated() {
        let script = r#"
            function filter(ctx)
                if ctx.user == nil then
                    return "1=0"
                end
                return ""
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("mycat_mylayer", script)]);
        let result = registry
            .call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10))
            .await;
        assert_eq!(result, Some("1=0".to_string()));
    }

    #[tokio::test]
    async fn filter_receives_groups_as_table() {
        let script = r#"
            function filter(ctx)
                for _, g in ipairs(ctx.groups) do
                    if g == "premium" then
                        return ""
                    end
                end
                return "public = true"
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("mycat_mylayer", script)]);

        let premium = registry
            .call_filter("mycat_mylayer", "mycat", &ctx_with_user("mylayer", "mycat", "alice", &["viewer", "premium"]))
            .await;
        assert_eq!(premium, Some(String::new()));

        let regular = registry
            .call_filter("mycat_mylayer", "mycat", &ctx_with_user("mylayer", "mycat", "bob", &["viewer"]))
            .await;
        assert_eq!(regular, Some("public = true".to_string()));
    }

    #[tokio::test]
    async fn filter_receives_empty_groups_table_when_unauthenticated() {
        let script = r#"
            function filter(ctx)
                return tostring(#ctx.groups)
            end
        "#;
        let registry = LuaPluginRegistry::from_scripts(&[("mycat_mylayer", script)]);
        let result = registry
            .call_filter("mycat_mylayer", "mycat", &ctx("mylayer", "mycat", 10))
            .await;
        assert_eq!(result, Some("0".to_string()));
    }
}
