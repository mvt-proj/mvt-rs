use crate::auth::Group;
use crate::get_cf_pool;
use crate::config::system_settings::bump_config_version;
use sqlx::{Row, sqlite::SqlitePool};

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

pub async fn create_group(group: &Group, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("INSERT INTO Groups (id, name, description) VALUES (?, ?, ?)")
        .bind(&group.id)
        .bind(&group.name)
        .bind(&group.description)
        .execute(pool)
        .await?;

    bump_config_version(pool).await?;

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

    bump_config_version(pool).await?;

    Ok(())
}

pub async fn delete_group(id: String, pool: Option<&SqlitePool>) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("DELETE FROM Groups WHERE id = ?")
        .bind(id)
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

    #[tokio::test]
    async fn delete_group_bumps_version() {
        let pool = in_memory_pool().await;
        delete_group("nonexistent".to_string(), Some(&pool)).await.unwrap();
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
