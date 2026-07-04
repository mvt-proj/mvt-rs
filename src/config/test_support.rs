#![cfg(test)]

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

/// In-memory SQLite pool with all migrations applied. `max_connections(1)` is
/// required: each connection to `sqlite::memory:` gets its own database, so a
/// multi-connection pool would migrate one DB and query another.
pub async fn in_memory_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}
