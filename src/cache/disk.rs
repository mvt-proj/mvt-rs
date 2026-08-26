use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, SystemTime};

use crate::{
    Catalog,
    error::{AppError, AppResult},
};
use bytes::Bytes;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone)]
pub struct DiskCache {
    pub cache_dir: PathBuf,
}

impl DiskCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        DiskCache { cache_dir }
    }

    pub async fn delete_cache_dir(&self, catalog: Catalog) {
        for layer in catalog.layers.iter() {
            if layer.delete_cache_on_start.unwrap_or(false) {
                let dir_path = Path::new(&self.cache_dir).join(&layer.name);

                if let Err(err) = tokio::fs::remove_dir_all(&dir_path).await {
                    tracing::warn!(
                        "Failed to delete the cache directory {:?}: {}",
                        &dir_path,
                        err
                    );
                } else {
                    tracing::warn!("Directory {:?} deleted successfully.", &dir_path);
                }
            }
        }
    }

    pub async fn delete_layer_cache(&self, layer_name: &String) {
        let dir_path = Path::new(&self.cache_dir).join(layer_name);

        if let Err(err) = tokio::fs::remove_dir_all(&dir_path).await {
            tracing::warn!(
                "Failed to delete the cache directory {:?}: {}",
                &dir_path,
                err
            );
        } else {
            tracing::warn!("Directory {:?} deleted successfully.", &dir_path);
        }
    }

    pub async fn get_cache(&self, tilepath: PathBuf, max_cache_age: u64) -> AppResult<Bytes> {
        if let Ok(metadata) = fs::metadata(&tilepath).await {
            let cache_modified = match metadata.modified() {
                Ok(modified_time) => modified_time,
                Err(_) => SystemTime::UNIX_EPOCH,
            };
            let cache_age = cache_modified
                .elapsed()
                .unwrap_or_else(|_| Duration::from_secs(0));

            let max_cache_age = Duration::from_secs(max_cache_age);
            if cache_age <= max_cache_age || max_cache_age == Duration::from_secs(0) {
                let mut tile = Vec::new();
                let mut file = File::open(&tilepath).await?;
                file.read_to_end(&mut tile).await?;
                return Ok(tile.into());
            }
            // Expired: treated as a miss without deleting the file here — a
            // per-request delete would block the response on disk I/O. The
            // file is either overwritten once the tile is regenerated and
            // re-cached, or reclaimed later by the periodic disk-cache
            // janitor (see `cleanup_expired`).
        }

        Err(AppError::CacheNotFound(
            "Tile not found in cache or expired".to_string(),
        ))
    }

    pub async fn write_tile_to_file(&self, tilepath: &PathBuf, tile: &[u8]) -> AppResult<()> {
        if let Some(parent) = tilepath.parent()
            && fs::metadata(parent).await.is_err()
        {
            fs::create_dir_all(parent).await?;
        }

        let mut file = File::create(tilepath).await?;
        file.write_all(tile).await?;
        file.flush().await?;

        Ok(())
    }

    /// Returns the current version counter for a layer.
    /// Stored at `{cache_dir}/.versions/{layer_name}` — outside the tile directory
    /// so it survives tile cache deletion.
    pub async fn get_layer_version(&self, layer_name: &str) -> u64 {
        let path = self.cache_dir.join(".versions").join(layer_name);
        match fs::read_to_string(&path).await {
            Ok(s) => s.trim().parse().unwrap_or(0),
            Err(_) => 0,
        }
    }

    /// Increments the version counter for a layer.
    pub async fn increment_layer_version(&self, layer_name: &str) {
        let dir = self.cache_dir.join(".versions");
        if fs::metadata(&dir).await.is_err() {
            if let Err(e) = fs::create_dir_all(&dir).await {
                tracing::warn!("Failed to create versions dir: {e}");
                return;
            }
        }
        let path = dir.join(layer_name);
        let current: u64 = match fs::read_to_string(&path).await {
            Ok(s) => s.trim().parse().unwrap_or(0),
            Err(_) => 0,
        };
        if let Err(e) = fs::write(&path, (current + 1).to_string()).await {
            tracing::warn!("Failed to write version for layer {layer_name}: {e}");
        }
    }

    /// Removes cached tiles older than each layer's `max_cache_age`. Layers with
    /// `max_cache_age == 0` never expire and are skipped. Meant to run
    /// periodically in the background so expired tiles that `get_cache` leaves
    /// in place don't accumulate indefinitely.
    pub async fn cleanup_expired(&self, catalog: &Catalog) {
        for layer in &catalog.layers {
            let max_cache_age = layer.max_cache_age.unwrap_or(0);
            if max_cache_age == 0 {
                continue;
            }
            let layer_dir = self
                .cache_dir
                .join(format!("{}_{}", layer.category.name, layer.name));
            remove_expired_in_dir(&layer_dir, Duration::from_secs(max_cache_age)).await;
        }
    }
}

/// Recursively removes files under `dir` whose modification time is older
/// than `max_cache_age`. Missing directories and unreadable entries are
/// skipped silently — this runs on a best-effort background schedule.
fn remove_expired_in_dir<'a>(
    dir: &'a Path,
    max_cache_age: Duration,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(_) => return,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };

            if file_type.is_dir() {
                remove_expired_in_dir(&path, max_cache_age).await;
                continue;
            }

            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if modified.elapsed().unwrap_or(Duration::ZERO) > max_cache_age
                && let Err(e) = fs::remove_file(&path).await
            {
                tracing::warn!("cache janitor: failed to remove {:?}: {e}", path);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{catalog::Layer, category::Category};

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mvt-rs-disk-cache-test-{name}-{}",
            std::process::id()
        ))
    }

    fn make_layer(category: &str, name: &str, max_cache_age: Option<u64>) -> Layer {
        Layer {
            id: "id".into(),
            category: Category {
                id: "cat-id".into(),
                name: category.into(),
                description: String::new(),
            },
            geometry: "points".into(),
            name: name.into(),
            alias: name.into(),
            description: String::new(),
            database_id: "default".into(),
            schema: "public".into(),
            table_name: "t".into(),
            fields: vec![],
            filter: None,
            srid: None,
            geom: None,
            label_layer: false,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age,
            max_records: None,
            published: true,
            url: None,
            groups: None,
        }
    }

    #[tokio::test]
    async fn get_cache_expired_returns_miss_without_deleting_file() {
        let dir = temp_dir("expired-no-delete");
        let cache = DiskCache::new(dir.clone());
        let tilepath = dir.join("tile.pbf");
        cache.write_tile_to_file(&tilepath, b"stale").await.unwrap();

        tokio::time::sleep(Duration::from_millis(1100)).await;

        let result = cache.get_cache(tilepath.clone(), 1).await;
        assert!(result.is_err());
        assert!(
            tilepath.exists(),
            "expired file must be left in place for the janitor to clean up"
        );

        fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn cleanup_expired_removes_only_stale_files_past_layer_ttl() {
        let dir = temp_dir("cleanup-expired");
        let cache = DiskCache::new(dir.clone());

        let stale_path = dir.join("cat_old").join("0").join("0").join("0.pbf");
        let fresh_path = dir.join("cat_new").join("0").join("0").join("0.pbf");
        let forever_path = dir.join("cat_forever").join("0").join("0").join("0.pbf");

        cache.write_tile_to_file(&stale_path, b"tile").await.unwrap();
        cache.write_tile_to_file(&fresh_path, b"tile").await.unwrap();
        cache
            .write_tile_to_file(&forever_path, b"tile")
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(1100)).await;

        let catalog = Catalog {
            layers: vec![
                make_layer("cat", "old", Some(1)),
                make_layer("cat", "new", Some(3600)),
                make_layer("cat", "forever", Some(0)),
            ],
        };

        cache.cleanup_expired(&catalog).await;

        assert!(
            !stale_path.exists(),
            "stale tile past its layer TTL should be removed"
        );
        assert!(
            fresh_path.exists(),
            "fresh tile within its layer TTL must survive"
        );
        assert!(
            forever_path.exists(),
            "max_cache_age = 0 means never expire, must survive"
        );

        fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn cleanup_expired_ignores_layers_with_no_cache_dir_yet() {
        let dir = temp_dir("cleanup-missing-dir");
        let cache = DiskCache::new(dir.clone());
        let catalog = Catalog {
            layers: vec![make_layer("cat", "nonexistent", Some(60))],
        };

        cache.cleanup_expired(&catalog).await;
    }
}
