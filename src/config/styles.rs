use crate::get_cf_pool;
use crate::{styles::Style, category::Category};
use sqlx::{sqlite::SqlitePool, Row};

pub async fn get_styles(pool: Option<&SqlitePool>) -> Result<Vec<Style>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query(
        r#"
        SELECT 
            s.*, 
            c.id AS category_id, 
            c.name AS category_name, 
            c.description AS category_description
        FROM 
            styles s
        LEFT JOIN 
            categories c 
        ON 
            s.category = c.id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut styles = Vec::new();

    for row in rows {
        let id: String = row.get("id");
        let category = Category {
            id: row.get("category_id"),
            name: row.get("category_name"),
            description: row.get("category_description"),
        };
        let name: String = row.get("name");
        let style: String = row.get("style");

        styles.push(Style {
            id,
            name,
            category,
            style,
        });
    }

    Ok(styles)
}

pub async fn get_style(id: &str, pool: Option<&SqlitePool>) -> Result<Style, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let row = sqlx::query(
        r#"
        SELECT 
            s.*, 
            c.id AS category_id, 
            c.name AS category_name, 
            c.description AS category_description
        FROM 
            styles s
        LEFT JOIN 
            categories c 
        ON 
            s.category = c.id
        WHERE 
            s.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    let category = Category {
        id: row.get("category_id"),
        name: row.get("category_name"),
        description: row.get("category_description"),
    };
    let name: String = row.get("name");
    let style: String = row.get("style");

    Ok(Style {
        id: row.get("id"),
        name,
        category,
        style,
    })
}

pub async fn create_style(
    style: Style,
    pool: Option<&SqlitePool>,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query(
        r#"
        INSERT INTO styles (id, name, category, style)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(style.id)
    .bind(style.name)
    .bind(style.category.id)
    .bind(style.style)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_style(
    style: Style,
    pool: Option<&SqlitePool>,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query(
        r#"
        UPDATE styles
        SET name = $1, category = $2, style = $3
        WHERE id = $4
        "#,
    )
    .bind(style.name)
    .bind(style.category.id)
    .bind(style.style)
    .bind(style.id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_style(id: &str, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query(
        r#"
        DELETE FROM styles
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}