use crate::get_cf_pool;
use crate::{catalog::Layer, category::Category};
use sqlx::{sqlite::SqlitePool, Row};

pub async fn get_layers(pool: Option<&SqlitePool>) -> Result<Vec<Layer>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let rows = sqlx::query(
        r#"
        SELECT 
            l.*, 
            c.id AS category_id, 
            c.name AS category_name, 
            c.description AS category_description
        FROM 
            layers l
        LEFT JOIN 
            categories c 
        ON 
            l.category = c.id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut layers = Vec::new();

    for row in rows {
        let id: String = row.get("id");
        let category = Category {
            id: row.get("category_id"),
            name: row.get("category_name"),
            description: row.get("category_description"),
        };
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
            category,
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
            id, category, geometry, name, alias, schema, table_name, fields, filter, srid, geom, 
            sql_mode, buffer, extent, zmin, zmax, zmax_do_not_simplify, 
            buffer_do_not_simplify, extent_do_not_simplify, clip_geom, 
            delete_cache_on_start, max_cache_age, published, url
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
        )",
    )
    .bind(&layer.id)
    .bind(&layer.category.id)
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

    let row = sqlx::query(
        r#"
        SELECT 
            l.*, 
            c.id AS category_id, 
            c.name AS category_name, 
            c.description AS category_description
        FROM 
            layers l
        LEFT JOIN 
            categories c 
        ON 
            l.category = c.id
        WHERE 
            l.id = ?
        "#,
    )
    .bind(layer_id)
    .fetch_one(pool)
    .await?;

    let fields: String = row.get("fields");
    let fields_vec: Vec<String> = fields.split(',').map(|s| s.trim().to_string()).collect();

    let id: String = row.get("id");
    let category = Category {
        id: row.get("category_id"),
        name: row.get("category_name"),
        description: row.get("category_description"),
    };
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
        category,
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
            category = ?, geometry = ?, name = ?, alias = ?, schema = ?, table_name = ?, fields = ?, 
            filter = ?, srid = ?, geom = ?, sql_mode = ?, buffer = ?, extent = ?, zmin = ?, 
            zmax = ?, zmax_do_not_simplify = ?, buffer_do_not_simplify = ?, 
            extent_do_not_simplify = ?, clip_geom = ?, delete_cache_on_start = ?, 
            max_cache_age = ?, published = ?, url = ? WHERE id = ?",
    )
    .bind(&layer.category.id)
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
