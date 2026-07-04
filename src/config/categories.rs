use crate::get_cf_pool;
use crate::models::category::Category;
use crate::config::system_settings::bump_config_version;
use sqlx::{Row, sqlite::SqlitePool};

pub async fn get_categories(pool: Option<&SqlitePool>) -> Result<Vec<Category>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query("SELECT * FROM categories ORDER BY name")
        .fetch_all(pool)
        .await?;

    let mut categories = Vec::new();

    for row in rows {
        let id: String = row.get("id");
        let name: String = row.get("name");
        let description: String = row.get("description");

        categories.push(Category {
            id,
            name,
            description,
        });
    }

    Ok(categories)
}

pub async fn create_category(
    pool: Option<&SqlitePool>,
    category: Category,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("INSERT INTO categories (id, name, description) VALUES (?, ?, ?)")
        .bind(&category.id)
        .bind(&category.name)
        .bind(&category.description)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
}

pub async fn get_category_by_id(
    pool: Option<&SqlitePool>,
    category_id: &str,
) -> Result<Category, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let row = sqlx::query("SELECT * FROM categories WHERE id = ?")
        .bind(category_id)
        .fetch_one(pool)
        .await?;

    let id: String = row.get("id");
    let name: String = row.get("name");
    let description: String = row.get("description");

    Ok(Category {
        id,
        name,
        description,
    })
}

pub async fn update_category(
    pool: Option<&SqlitePool>,
    category: Category,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("UPDATE categories SET name = ?, description = ? WHERE id = ?")
        .bind(&category.name)
        .bind(&category.description)
        .bind(&category.id)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
}

pub async fn delete_category(
    pool: Option<&SqlitePool>,
    category_id: &str,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(category_id)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;

    fn cat(id: &str) -> Category {
        Category { id: id.into(), name: format!("name-{id}"), description: "d".into() }
    }

    #[tokio::test]
    async fn create_category_bumps_version() {
        let pool = in_memory_pool().await;
        create_category(Some(&pool), cat("c1")).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn update_category_bumps_version() {
        let pool = in_memory_pool().await;
        create_category(Some(&pool), cat("c1")).await.unwrap();
        update_category(Some(&pool), cat("c1")).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn delete_category_bumps_version() {
        let pool = in_memory_pool().await;
        delete_category(Some(&pool), "nonexistent").await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
