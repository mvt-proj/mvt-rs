use std::path::{Path, PathBuf};
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
            if layer.delete_cache_on_start.unwrap() {
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
            if cache_age > max_cache_age && max_cache_age != Duration::from_secs(0) {
                fs::remove_file(&tilepath).await?;
            } else {
                let mut tile = Vec::new();
                let mut file = File::open(&tilepath).await?;
                file.read_to_end(&mut tile).await?;
                return Ok(tile.into());
            }
        }

        Err(AppError::CacheNotFount("aa".to_string()))
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
}
