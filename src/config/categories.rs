use crate::get_cf_pool;
use crate::models::category::Category;
use sqlx::{sqlite::SqlitePool, Row};

pub async fn get_categories(pool: Option<&SqlitePool>) -> Result<Vec<Category>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query("SELECT * FROM categories")
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

    Ok(())
}
