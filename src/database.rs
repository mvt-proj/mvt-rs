use serde::Serialize;
use sqlx::{FromRow, PgPool};

use crate::{error::AppResult, get_db_pool};

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

    let sql = format!(
        r#"
            SELECT
                table_name name,
                column_name geometry
            FROM
                information_schema.columns
            WHERE
                table_schema = '{schema}'
                AND data_type = 'USER-DEFINED'
                AND udt_name = 'geometry'
            ORDER BY
                table_name;
        "#
    );

    let data = sqlx::query_as::<_, Table>(&sql).fetch_all(&pg_pool).await?;
    Ok(data)
}

pub async fn query_fields(schema: String, table: String) -> AppResult<Vec<Field>> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = format!(
        r#"
            SELECT
                column_name name,
                udt_name udt
            FROM
                information_schema.columns
            WHERE table_schema = '{schema}'
              AND table_name = '{table}';
        "#
    );

    let data = sqlx::query_as::<_, Field>(&sql).fetch_all(&pg_pool).await?;
    Ok(data)
}

pub async fn query_srid(schema: String, table: String, geometry: String) -> AppResult<Srid> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = format!(
        r#"
            SELECT Find_SRID('{schema}', '{table}', '{geometry}') AS name
            FROM {schema}.{table}
            LIMIT 1;
        "#
    );

    let data = sqlx::query_as::<_, Srid>(&sql).fetch_one(&pg_pool).await?;
    Ok(data)
}

pub async fn query_extent(schema: String, table: String, geometry: String) -> AppResult<Extent> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = format!(
        r#"
            SELECT
              ST_XMin(ST_Extent(ST_Transform({geometry}, 4326))) AS xmin,
              ST_YMin(ST_Extent(ST_Transform({geometry}, 4326))) AS ymin,
              ST_XMax(ST_Extent(ST_Transform({geometry}, 4326))) AS xmax,
              ST_YMax(ST_Extent(ST_Transform({geometry}, 4326))) AS ymax
            FROM {schema}.{table};
        "#
    );
    let data = sqlx::query_as::<_, Extent>(&sql)
        .fetch_one(&pg_pool)
        .await?;
    Ok(data)
}
