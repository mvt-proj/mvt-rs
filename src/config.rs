use crate::auth::{Group, User};
use crate::get_cf_pool;
use sqlx::{sqlite::SqlitePool, Row};

pub async fn get_users(pool: Option<&SqlitePool>) -> Result<Vec<User>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query("SELECT id, username, email, password, groups FROM Users")
        .fetch_all(pool)
        .await?;

    let mut users = Vec::new();

    for row in rows {
        let id: String = row.get("id");
        let username: String = row.get("username");
        let email: String = row.get("email");
        let password: String = row.get("password");
        let group_ids: String = row.get("groups");

        let group_ids_vec: Vec<&str> = group_ids.split(',').collect();
        let group_ids_sql = group_ids_vec
            .iter()
            .map(|id| format!("'{}'", id))
            .collect::<Vec<String>>()
            .join(",");

        let groups_query = format!(
            "SELECT id, name, description FROM Groups WHERE id IN ({})",
            group_ids_sql
        );

        println!("groups_query: {}", groups_query);

        let groups_rows = sqlx::query(&groups_query).fetch_all(pool).await?;
        let mut groups = Vec::new();

        for group_row in groups_rows {
            let id: String = group_row.get("id");
            let name: String = group_row.get("name");
            let description: String = group_row.get("description");

            groups.push(Group {
                id,
                name,
                description,
            });
        }

        users.push(User {
            id,
            username,
            email,
            password,
            groups,
        });
    }

    Ok(users)
}

pub async fn get_groups(pool: Option<&SqlitePool>) -> Result<Vec<Group>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query("SELECT id, name, description FROM Groups")
        .fetch_all(pool)
        .await?;

    let mut groups = Vec::new();

    for row in rows {
        let id: String = row.get("id");
        let name: String = row.get("name");
        let description: String = row.get("description");

        groups.push(Group {
            id,
            name,
            description,
        });
    }

    Ok(groups)
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

pub async fn create_group(group: &Group, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("INSERT INTO Groups (id, name, description) VALUES (?, ?, ?)")
        .bind(&group.id)
        .bind(&group.name)
        .bind(&group.description)
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

pub async fn update_group(
    id: String,
    group: &Group,
    pool: Option<&SqlitePool>,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("UPDATE Groups SET name = ?, description = ? WHERE id = ?")
        .bind(&group.name)
        .bind(&group.description)
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

pub async fn delete_group(id: String, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("DELETE FROM Groups WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}
