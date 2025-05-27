use serde::Serialize;
use sqlx::{FromRow, PgPool};

use crate::{error::AppResult, get_db_pool, models::catalog::Layer};

#[derive(FromRow, Serialize, Debug)]
pub struct Schema {
    pub name: String,
}

#[derive(FromRow, Serialize, Debug)]
pub struct Table {
    pub name: String,
    pub geometry: String,
}

#[derive(FromRow, Serialize, Debug)]
pub struct Field {
    pub name: String,
    pub udt: String,
}

#[derive(FromRow, Serialize, Debug)]
pub struct Srid {
    pub name: i32,
}

#[derive(FromRow, Serialize, Debug)]
pub struct Extent {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

pub async fn query_schemas() -> AppResult<Vec<Schema>> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = r#"
            SELECT schema_name name
            FROM information_schema.schemata
            ORDER BY schema_name;
        "#
    .to_string();

    let data = sqlx::query_as::<_, Schema>(&sql)
        .fetch_all(&pg_pool)
        .await?;
    Ok(data)
}

pub async fn query_tables(schema: String) -> AppResult<Vec<Table>> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = r#"
        SELECT
            c.relname AS name,
            a.attname AS geometry
        FROM
            pg_attribute a
        JOIN
            pg_class c ON a.attrelid = c.oid
        JOIN
            pg_namespace n ON c.relnamespace = n.oid
        JOIN
            pg_type t ON a.atttypid = t.oid
        WHERE
            n.nspname = $1  -- schema
            AND t.typname = 'geometry'
            AND a.attnum > 0
            AND NOT a.attisdropped
            AND c.relkind IN ('r', 'v', 'm')  -- r = table, v = view, m = materialized view
        ORDER BY
            c.relname;

    "#;

    let data = sqlx::query_as::<_, Table>(sql)
        .bind(schema)
        .fetch_all(&pg_pool)
        .await?;

    Ok(data)
}

pub async fn query_fields(schema: String, table: String) -> AppResult<Vec<Field>> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = r#"
        SELECT
            a.attname AS name,
            t.typname AS udt
        FROM
            pg_attribute a
        JOIN
            pg_class c ON a.attrelid = c.oid
        JOIN
            pg_namespace n ON c.relnamespace = n.oid
        JOIN
            pg_type t ON a.atttypid = t.oid
        WHERE
            n.nspname = $1
            AND c.relname = $2
            AND a.attnum > 0
            AND NOT a.attisdropped
            AND c.relkind IN ('r', 'v', 'm')  -- r = table, v = view, m = materialized view
        ORDER BY
            a.attnum;

    "#;

    let data = sqlx::query_as::<_, Field>(sql)
        .bind(schema)
        .bind(table)
        .fetch_all(&pg_pool)
        .await?;

    Ok(data)
}

pub async fn query_srid(schema: String, table: String, geometry: String) -> AppResult<Srid> {
    let pg_pool: PgPool = get_db_pool().clone();
    let full_table = format!("{}.{}", schema, table);
    let sql = r#"
        SELECT Find_SRID($1, $2, $3) AS name
        FROM {}
        LIMIT 1;
    "#;

    let sql = sql.replace("{}", &full_table);
    let data = sqlx::query_as::<_, Srid>(&sql)
        .bind(&schema)
        .bind(&table)
        .bind(&geometry)
        .fetch_one(&pg_pool)
        .await?;

    Ok(data)
}

pub async fn query_extent(layer: &Layer) -> AppResult<Extent> {
    let pg_pool: PgPool = get_db_pool().clone();
    let schema = &layer.schema;
    let table = &layer.table_name;
    let geometry = layer.get_geom();

    let full_table = format!("{}.{}", schema, table);

    let sql = format!(
        r#"
        SELECT
          COALESCE(ST_XMin(ST_Extent(ST_Transform({geometry}, 4326))), -180) AS xmin,
          COALESCE(ST_YMin(ST_Extent(ST_Transform({geometry}, 4326))), -90) AS ymin,
          COALESCE(ST_XMax(ST_Extent(ST_Transform({geometry}, 4326))), 180) AS xmax,
          COALESCE(ST_YMax(ST_Extent(ST_Transform({geometry}, 4326))), 90) AS ymax
        FROM {};
        "#,
        full_table
    );

    let data = sqlx::query_as::<_, Extent>(&sql)
        .fetch_one(&pg_pool)
        .await?;
    Ok(data)
}
