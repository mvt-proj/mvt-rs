CREATE TABLE IF NOT EXISTS plugins_config (
    plugin_key TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1
);
