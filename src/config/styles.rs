use crate::get_cf_pool;
use crate::models::{category::Category, styles::Style};
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
        let description: String = row.get("description");
        let style: String = row.get("style");

        styles.push(Style {
            id,
            name,
            description,
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
    let description: String = row.get("description");
    let style: String = row.get("style");

    Ok(Style {
        id: row.get("id"),
        name,
        description,
        category,
        style,
    })
}

pub async fn get_style_by_category_and_name(
    category: &str,
    name: &str,
    pool: Option<&SqlitePool>,
) -> Result<Style, sqlx::Error> {
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
        LEFT JOIN categories c ON s.category = c.id
        WHERE
            c.name = $1
            AND s.name = $2
        "#,
    )
    .bind(category)
    .bind(name)
    .fetch_one(pool)
    .await?;

    let category = Category {
        id: row.get("category_id"),
        name: row.get("category_name"),
        description: row.get("category_description"),
    };
    let name: String = row.get("name");
    let description: String = row.get("description");
    let style: String = row.get("style");

    Ok(Style {
        id: row.get("id"),
        name,
        description,
        category,
        style,
    })
}

pub async fn create_style(style: Style, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query(
        r#"
        INSERT INTO styles (id, name, category, description, style)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(style.id)
    .bind(style.name)
    .bind(style.category.id)
    .bind(style.description)
    .bind(style.style)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_style(style: Style, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query(
        r#"
        UPDATE styles
        SET name = $1, category = $2, style = $3, description = $4
        WHERE id = $5
        "#,
    )
    .bind(style.name)
    .bind(style.category.id)
    .bind(style.style)
    .bind(style.description)
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
