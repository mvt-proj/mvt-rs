use std::collections::HashMap;
use crate::auth::{Group, User};
use crate::get_cf_pool;
use sqlx::{sqlite::SqlitePool, Row};

pub async fn get_users(pool: Option<&SqlitePool>) -> Result<Vec<User>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query(
        "SELECT 
            u.id as user_id, 
            u.username, 
            u.email, 
            u.password, 
            g.id as group_id, 
            g.name as group_name, 
            g.description as group_description 
         FROM Users u
         LEFT JOIN Groups g ON ',' || u.groups || ',' LIKE '%,' || g.id || ',%'",
    )
    .fetch_all(pool)
    .await?;

    let mut users_map: HashMap<String, User> = HashMap::new();

    for row in rows {
        let user_id: String = row.get("user_id");
        let username: String = row.get("username");
        let email: String = row.get("email");
        let password: String = row.get("password");
        let group_id: Option<String> = row.get("group_id");
        let group_name: Option<String> = row.get("group_name");
        let group_description: Option<String> = row.get("group_description");

        let user = users_map.entry(user_id.clone()).or_insert(User {
            id: user_id.clone(),
            username,
            email,
            password,
            groups: Vec::new(),
        });

        if let (Some(id), Some(name), Some(description)) = (group_id, group_name, group_description)
        {
            user.groups.push(Group {
                id,
                name,
                description,
            });
        }
    }

    Ok(users_map.into_values().collect())
}

pub async fn create_user(user: &User, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let group_ids_str = user
        .groups
        .iter()
        .map(|group| group.id.clone().to_string())
        .collect::<Vec<String>>()
        .join(",");

    sqlx::query("INSERT INTO Users (id, username, email, password, groups) VALUES (?, ?, ?, ?, ?)")
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.password)
        .bind(group_ids_str)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_user(
    id: String,
    user: &User,
    pool: Option<&SqlitePool>,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let group_ids_str = user
        .groups
        .iter()
        .map(|group| group.id.clone().to_string())
        .collect::<Vec<String>>()
        .join(",");

    sqlx::query("UPDATE Users SET username = ?, email = ?, password = ?, groups = ? WHERE id = ?")
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.password)
        .bind(group_ids_str)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}


pub async fn delete_user(id: String, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("DELETE FROM Users WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}


