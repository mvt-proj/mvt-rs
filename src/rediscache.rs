use crate::{error::AppResult, Catalog};
use bb8_redis::{
    bb8,
    // redis::{AsyncCommands, RedisResult},
    redis::AsyncCommands,
    RedisConnectionManager,
};
use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct RedisCache {
    pub conn_info: String,
    pool: bb8::Pool<RedisConnectionManager>,
}

impl RedisCache {
    pub async fn new(conn_info: String) -> AppResult<Self> {
        let manager = RedisConnectionManager::new(conn_info.clone())?;
        let pool = bb8::Pool::builder().build(manager).await?;
        Ok(RedisCache { conn_info, pool })
    }

    pub async fn delete_cache(&self, catalog: Catalog) -> AppResult<()> {
        for layer in catalog.layers.iter() {
            if layer.delete_cache_on_start.unwrap_or(false) {
                let mut conn = self.pool.get().await?;
                let key_pattern = format!("{}:*", layer.name);
                let keys: Vec<String> = conn.keys(key_pattern).await?;

                if !keys.is_empty() {
                    for key in keys {
                        conn.del::<&str, ()>(&key).await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn exists_key(&self, key: String) -> AppResult<bool> {
        let mut conn = self.pool.get().await?;
        let ret: bool = conn.exists(&key).await?;
        Ok(ret)
    }

    pub async fn get_cache(&self, key: String) -> AppResult<Bytes> {
        let mut conn = self.pool.get().await?;
        let retrieved_data: Bytes = conn.get(&key).await?;
        Ok(retrieved_data)
    }

    pub async fn write_tile_to_cache(
        &self,
        key: String,
        tile: &[u8],
        max_cache_age: u64,
    ) -> AppResult<()> {
        let mut conn = self.pool.get().await?;
        conn.set::<&str, Vec<u8>, ()>(&key, tile.to_vec()).await?;
        if max_cache_age != 0 {
            conn.expire::<&str, ()>(&key, max_cache_age.try_into()?)
                .await?;
        }

        Ok(())
    }
}
