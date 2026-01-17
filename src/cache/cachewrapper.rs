use super::disk::DiskCache;
use super::redis::RedisCache;
use crate::{Catalog, error::AppResult};
use bytes::Bytes;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CacheMode {
    Redis(RedisCache),
    Disk(DiskCache),
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

    pub async fn initialize_cache(
        redis_conn: Option<String>,
        disk_cache_dir: PathBuf,
        catalog: Catalog,
    ) -> AppResult<CacheWrapper> {
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
        }
    }

    pub async fn delete_cache(&self, catalog: Catalog) -> AppResult<()> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.delete_cache(catalog).await,
            CacheMode::Disk(disk_cache) => {
                disk_cache.delete_cache_dir(catalog).await;
                Ok(())
            }
        }
    }

    pub async fn delete_layer_cache(&self, layer_name: &String) -> AppResult<()> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.delete_layer_cache(layer_name).await,
            CacheMode::Disk(disk_cache) => {
                disk_cache.delete_layer_cache(layer_name).await;
                Ok(())
            }
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
        }
    }

    pub async fn exists_key(&self, key: String) -> AppResult<bool> {
        match &self.mode {
            CacheMode::Redis(redis_cache) => redis_cache.exists_key(key).await,
            CacheMode::Disk(_) => Ok(false),
        }
    }
}
