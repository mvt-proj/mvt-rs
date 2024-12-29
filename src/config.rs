use std::collections::HashMap;

use crate::auth::{Group, User};
use crate::catalog::Layer;
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

pub async fn get_layers(pool: Option<&SqlitePool>) -> Result<Vec<Layer>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query("SELECT * FROM layers").fetch_all(pool).await?;

    let mut layers = Vec::new();

    for row in rows {
        let id: String = row.get("id");
        let geometry: String = row.get("geometry");
        let name: String = row.get("name");
        let alias: String = row.get("alias");
        let schema: String = row.get("schema");
        let table_name: String = row.get("table_name");
        let fields: String = row.get("fields");
        let filter: Option<String> = row.get("filter");
        let srid: Option<i32> = row.get("srid");
        let geom: Option<String> = row.get("geom");
        let sql_mode: Option<String> = row.get("sql_mode");
        let buffer: Option<i32> = row.get("buffer");
        let extent: Option<i32> = row.get("extent");
        let zmin: Option<i32> = row.get("zmin");
        let zmax: Option<i32> = row.get("zmax");
        let zmax_do_not_simplify: Option<i32> = row.get("zmax_do_not_simplify");
        let buffer_do_not_simplify: Option<i32> = row.get("buffer_do_not_simplify");
        let extent_do_not_simplify: Option<i32> = row.get("extent_do_not_simplify");
        let clip_geom: Option<bool> = row.get("clip_geom");
        let delete_cache_on_start: Option<bool> = row.get("delete_cache_on_start");
        let max_cache_age: Option<i64> = row.get("max_cache_age");
        let published: bool = row.get("published");
        let url: Option<String> = row.get("url");

        // Convertir el campo fields a Vec<String>
        let fields_vec: Vec<String> = fields.split(',').map(|s| s.trim().to_string()).collect();

        layers.push(Layer {
            id,
            geometry,
            name,
            alias,
            schema,
            table_name,
            fields: fields_vec,
            filter,
            srid: srid.map(|v| v as u32),
            geom,
            sql_mode,
            buffer: buffer.map(|v| v as u32),
            extent: extent.map(|v| v as u32),
            zmin: zmin.map(|v| v as u32),
            zmax: zmax.map(|v| v as u32),
            zmax_do_not_simplify: zmax_do_not_simplify.map(|v| v as u32),
            buffer_do_not_simplify: buffer_do_not_simplify.map(|v| v as u32),
            extent_do_not_simplify: extent_do_not_simplify.map(|v| v as u32),
            clip_geom,
            delete_cache_on_start,
            max_cache_age: max_cache_age.map(|v| v as u64),
            published,
            url,
        });
    }

    Ok(layers)
}

pub async fn create_layer(pool: Option<&SqlitePool>, layer: Layer) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let fields = layer.fields.join(",");

    sqlx::query(
        "INSERT INTO layers (
            id, geometry, name, alias, schema, table_name, fields, filter, srid, geom, 
            sql_mode, buffer, extent, zmin, zmax, zmax_do_not_simplify, 
            buffer_do_not_simplify, extent_do_not_simplify, clip_geom, 
            delete_cache_on_start, max_cache_age, published, url
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
        )",
    )
    .bind(&layer.id)
    .bind(&layer.geometry)
    .bind(&layer.name)
    .bind(&layer.alias)
    .bind(&layer.schema)
    .bind(&layer.table_name)
    .bind(fields)
    .bind(&layer.filter)
    .bind(layer.srid)
    .bind(&layer.geom)
    .bind(&layer.sql_mode)
    .bind(layer.buffer)
    .bind(layer.extent)
    .bind(layer.zmin)
    .bind(layer.zmax)
    .bind(layer.zmax_do_not_simplify)
    .bind(layer.buffer_do_not_simplify)
    .bind(layer.extent_do_not_simplify)
    .bind(layer.clip_geom)
    .bind(layer.delete_cache_on_start)
    .bind(layer.max_cache_age.map(|v| v as i64))
    .bind(layer.published)
    .bind(&layer.url)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_layer_by_id(
    pool: Option<&SqlitePool>,
    layer_id: &str,
) -> Result<Layer, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let row = sqlx::query("SELECT * FROM layers WHERE id = ?")
        .bind(layer_id)
        .fetch_one(pool)
        .await?;

    let fields: String = row.get("fields");
    let fields_vec: Vec<String> = fields.split(',').map(|s| s.trim().to_string()).collect();

    let id: String = row.get("id");
    let geometry: String = row.get("geometry");
    let name: String = row.get("name");
    let alias: String = row.get("alias");
    let schema: String = row.get("schema");
    let table_name: String = row.get("table_name");
    let filter: Option<String> = row.get("filter");
    let srid: Option<i32> = row.get("srid");
    let geom: Option<String> = row.get("geom");
    let sql_mode: Option<String> = row.get("sql_mode");
    let buffer: Option<i32> = row.get("buffer");
    let extent: Option<i32> = row.get("extent");
    let zmin: Option<i32> = row.get("zmin");
    let zmax: Option<i32> = row.get("zmax");
    let zmax_do_not_simplify: Option<i32> = row.get("zmax_do_not_simplify");
    let buffer_do_not_simplify: Option<i32> = row.get("buffer_do_not_simplify");
    let extent_do_not_simplify: Option<i32> = row.get("extent_do_not_simplify");
    let clip_geom: Option<bool> = row.get("clip_geom");
    let delete_cache_on_start: Option<bool> = row.get("delete_cache_on_start");
    let max_cache_age: Option<i64> = row.get("max_cache_age");
    let published: bool = row.get("published");
    let url: Option<String> = row.get("url");

    Ok(Layer {
        id,
        geometry,
        name,
        alias,
        schema,
        table_name,
        fields: fields_vec,
        filter,
        srid: srid.map(|v| v as u32),
        geom,
        sql_mode,
        buffer: buffer.map(|v| v as u32),
        extent: extent.map(|v| v as u32),
        zmin: zmin.map(|v| v as u32),
        zmax: zmax.map(|v| v as u32),
        zmax_do_not_simplify: zmax_do_not_simplify.map(|v| v as u32),
        buffer_do_not_simplify: buffer_do_not_simplify.map(|v| v as u32),
        extent_do_not_simplify: extent_do_not_simplify.map(|v| v as u32),
        clip_geom,
        delete_cache_on_start,
        max_cache_age: max_cache_age.map(|v| v as u64),
        published,
        url,
    })
}

pub async fn update_layer(pool: Option<&SqlitePool>, layer: Layer) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let fields = layer.fields.join(",");

    sqlx::query(
        "UPDATE layers SET 
            geometry = ?, name = ?, alias = ?, schema = ?, table_name = ?, fields = ?, 
            filter = ?, srid = ?, geom = ?, sql_mode = ?, buffer = ?, extent = ?, zmin = ?, 
            zmax = ?, zmax_do_not_simplify = ?, buffer_do_not_simplify = ?, 
            extent_do_not_simplify = ?, clip_geom = ?, delete_cache_on_start = ?, 
            max_cache_age = ?, published = ?, url = ? WHERE id = ?",
    )
    .bind(&layer.geometry)
    .bind(&layer.name)
    .bind(&layer.alias)
    .bind(&layer.schema)
    .bind(&layer.table_name)
    .bind(fields)
    .bind(&layer.filter)
    .bind(layer.srid)
    .bind(&layer.geom)
    .bind(&layer.sql_mode)
    .bind(layer.buffer)
    .bind(layer.extent)
    .bind(layer.zmin)
    .bind(layer.zmax)
    .bind(layer.zmax_do_not_simplify)
    .bind(layer.buffer_do_not_simplify)
    .bind(layer.extent_do_not_simplify)
    .bind(layer.clip_geom)
    .bind(layer.delete_cache_on_start)
    .bind(layer.max_cache_age.map(|v| v as i64))
    .bind(layer.published)
    .bind(&layer.url)
    .bind(&layer.id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_layer(pool: Option<&SqlitePool>, layer_id: &str) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    sqlx::query("DELETE FROM layers WHERE id = ?")
        .bind(layer_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn switch_layer_published(
    pool: Option<&SqlitePool>,
    layer_id: &str,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let row = sqlx::query("SELECT published FROM layers WHERE id = ?")
        .bind(layer_id)
        .fetch_one(pool)
        .await?;

    let published: bool = row.get("published");

    sqlx::query("UPDATE layers SET published = ? WHERE id = ?")
        .bind(!published)
        .bind(layer_id)
        .execute(pool)
        .await?;

    Ok(())
}
