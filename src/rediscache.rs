use crate::Catalog;
use bb8_redis::{
    bb8,
    redis::{AsyncCommands, RedisResult},
    RedisConnectionManager,
};
use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct RedisCache {
    pub conn_info: String,
    pool: bb8::Pool<RedisConnectionManager>,
}

impl RedisCache {
    pub async fn new(conn_info: String) -> Self {
        let manager = RedisConnectionManager::new(conn_info.clone()).unwrap();
        let pool = bb8::Pool::builder().build(manager).await.unwrap();

        RedisCache { conn_info, pool }
    }

    pub async fn delete_cache(&self, catalog: Catalog) {
        for layer in catalog.layers.iter() {
            if layer.delete_cache_on_start.unwrap() {
                let mut conn = self.pool.get().await.unwrap();
                let key_pattern = format!("{}:*", layer.name);
                let keys: Vec<String> = conn.keys(key_pattern).await.unwrap();

                if !keys.is_empty() {
                    for key in keys {
                        let _ = conn.del::<&str, ()>(&key).await.unwrap();
                    }
                }
            }
        }
    }

    pub async fn exists_key(&self, key: String) -> RedisResult<bool> {
        let mut conn = self.pool.get().await.unwrap();
        let ret: bool = conn.exists(&key).await?;
        Ok(ret)
    }

    pub async fn get_cache(&self, key: String) -> RedisResult<Bytes> {
        let mut conn = self.pool.get().await.unwrap();
        let retrieved_data: Vec<u8> = conn.get(&key).await?;
        Ok(retrieved_data.into())
    }

    pub async fn write_tile_to_cache(
        &self,
        key: String,
        tile: &[u8],
        max_cache_age: u64,
    ) -> RedisResult<()> {
        let mut conn = self.pool.get().await.unwrap();
        conn.set::<&str, Vec<u8>, ()>(&key, tile.to_vec()).await?;
        if max_cache_age != 0 {
            conn.expire::<&str, ()>(&key, max_cache_age.try_into().unwrap())
                .await?;
        }

        Ok(())
    }
}
