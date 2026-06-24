-- Global config version counter for multi-instance in-memory state sync.
-- Bumped on every config write; polled by the config watcher in each instance.
INSERT OR IGNORE INTO system_settings (key, value) VALUES ('config_version', '0');
