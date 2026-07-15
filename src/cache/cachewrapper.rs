use super::disk::DiskCache;
use super::redis::RedisCache;
use crate::{Catalog, error::AppResult};
use bytes::Bytes;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CacheMode {
    Redis(RedisCache),
    Disk(DiskCache),
    Disabled,
}

#[derive(Debug, Clone)]
pub struct CacheWrapper {
    mode: CacheMode,
}

impl CacheWrapper {
    pub fn new_redis(redis_cache: RedisCache) -> Self {
        CacheWrapper {
            mode: CacheMode::Redis(redis_cache),
        }
    }

    pub fn new_disk(disk_cache: DiskCache) -> Self {
        CacheWrapper {
            mode: CacheMode::Disk(disk_cache),
        }
    }

    pub fn new_disabled() -> Self {
        CacheWrapper {
            mode: CacheMode::Disabled,
        }
    }

    pub async fn initialize_cache(
        redis_conn: Option<String>,
        disk_cache_dir: PathBuf,
        catalog: Catalog,
        disabled: bool,
    ) -> AppResult<CacheWrapper> {
        if disabled {
            return Ok(CacheWrapper::new_disabled());
        }

        if let Some(redis_conn) = redis_conn
            && !redis_conn.is_empty()
        {
            let redis_cache = RedisCache::new(redis_conn).await?;
            redis_cache.delete_cache(catalog.clone()).await?;
            return Ok(CacheWrapper::new_redis(redis_cache));
        }

        let disk_cache = DiskCache::new(disk_cache_dir);
        disk_cache.delete_cache_dir(catalog).await;
        Ok(CacheWrapper::new_disk(disk_cache))
    }

    pub fn cache_dir(&self) -> PathBuf {
        match &self.mode {
            CacheMode::Disk(disk_cache) => disk_cache.cache_dir.clone(),
            CacheMode::Redis(_) => PathBuf::new(),
            CacheMode::Disabled => PathBuf::new(),
        }
    }

    pub async fn delete_cache(&self, catalog: Catalog) -> AppResult<()> {
        if matches!(self.mode, CacheMode::Disabled) {
            return Ok(());
        }
        // Increment version for affected layers before clearing tiles, so any
        // in-flight requests that complete after this point get a fresh ETag.
        for layer in catalog.layers.iter() {
            if layer.delete_cache_on_start.unwrap_or(false) {
                let key = format!("{}_{}", layer.category.name, layer.name);
                self.increment_layer_version(&key).await;
            }
        }
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.delete_cache(catalog).await,
            CacheMode::Disk(disk_cache) => {
                disk_cache.delete_cache_dir(catalog).await;
                Ok(())
            }
            CacheMode::Disabled => Ok(()),
        }
    }

    pub async fn delete_layer_cache(&self, layer_name: &String) -> AppResult<()> {
        if matches!(self.mode, CacheMode::Disabled) {
            return Ok(());
        }
        self.increment_layer_version(layer_name).await;
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.delete_layer_cache(layer_name).await,
            CacheMode::Disk(disk_cache) => {
                disk_cache.delete_layer_cache(layer_name).await;
                Ok(())
            }
            CacheMode::Disabled => Ok(()),
        }
    }

    /// Returns the current version counter for a layer.
    pub async fn get_layer_version(&self, layer_name: &str) -> u64 {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.get_layer_version(layer_name).await,
            CacheMode::Disk(disk_cache) => disk_cache.get_layer_version(layer_name).await,
            CacheMode::Disabled => 0,
        }
    }

    /// Increments the version counter for a layer (called on cache invalidation).
    pub async fn increment_layer_version(&self, layer_name: &str) {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.increment_layer_version(layer_name).await,
            CacheMode::Disk(disk_cache) => disk_cache.increment_layer_version(layer_name).await,
            CacheMode::Disabled => {}
        }
    }

    pub async fn get_tile(
        &self,
        name: &str,
        z: u32,
        x: u32,
        y: u32,
        max_cache_age: u64,
    ) -> Option<Bytes> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => {
                let key = format!("{name}:{z}:{x}:{y}");
                redis_cache.get_cache(key).await.ok()
            }
            CacheMode::Disk(disk_cache) => {
                let tilefolder = disk_cache
                    .cache_dir
                    .join(name)
                    .join(z.to_string())
                    .join(x.to_string());
                let tilepath = tilefolder.join(y.to_string()).with_extension("pbf");
                disk_cache.get_cache(tilepath, max_cache_age).await.ok()
            }
            CacheMode::Disabled => None,
        }
    }

    pub async fn write_tile(
        &self,
        name: &str,
        z: u32,
        x: u32,
        y: u32,
        tile: &[u8],
        max_cache_age: u64,
    ) -> AppResult<()> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => {
                let key = format!("{name}:{z}:{x}:{y}");
                redis_cache
                    .write_tile_to_cache(key, tile, max_cache_age)
                    .await
            }
            CacheMode::Disk(disk_cache) => {
                let tilefolder = disk_cache
                    .cache_dir
                    .join(name)
                    .join(z.to_string())
                    .join(x.to_string());
                let tilepath = tilefolder.join(y.to_string()).with_extension("pbf");
                disk_cache.write_tile_to_file(&tilepath, tile).await
            }
            CacheMode::Disabled => Ok(()),
        }
    }

    pub async fn exists_key(&self, key: String) -> AppResult<bool> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.exists_key(key).await,
            CacheMode::Disk(_) => Ok(false),
            CacheMode::Disabled => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_catalog() -> Catalog {
        Catalog { layers: vec![] }
    }

    #[tokio::test]
    async fn disabled_mode_write_then_get_returns_none() {
        let wrapper = CacheWrapper::new_disabled();
        wrapper
            .write_tile("layer", 1, 2, 3, b"tile-bytes", 0)
            .await
            .expect("write_tile should be a no-op success");

        let tile = wrapper.get_tile("layer", 1, 2, 3, 0).await;
        assert!(tile.is_none());
    }

    #[tokio::test]
    async fn disabled_mode_delete_and_version_are_noop() {
        let wrapper = CacheWrapper::new_disabled();

        wrapper
            .delete_cache(empty_catalog())
            .await
            .expect("delete_cache no-op");
        wrapper
            .delete_layer_cache(&"layer".to_string())
            .await
            .expect("delete_layer_cache no-op");

        assert_eq!(wrapper.get_layer_version("layer").await, 0);
        wrapper.increment_layer_version("layer").await;
        assert_eq!(wrapper.get_layer_version("layer").await, 0);

        assert!(!wrapper.exists_key("key".to_string()).await.unwrap());
        assert_eq!(wrapper.cache_dir(), PathBuf::new());
    }

    #[tokio::test]
    async fn initialize_cache_disabled_skips_disk_setup() {
        let untouched_dir =
            std::env::temp_dir().join("mvt-rs-test-no-cache-untouched");

        let wrapper = CacheWrapper::initialize_cache(
            None,
            untouched_dir.clone(),
            empty_catalog(),
            true,
        )
        .await
        .expect("disabled cache should initialize without a backend");

        assert_eq!(wrapper.cache_dir(), PathBuf::new());
        assert!(!untouched_dir.exists());
    }
}
