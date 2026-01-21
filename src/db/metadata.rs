use crate::{error::AppResult, get_db_pool, models::catalog::Layer};
use serde::Serialize;
use sqlx::{FromRow, PgPool};

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

fn escape_identifier(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

pub async fn query_schemas() -> AppResult<Vec<Schema>> {
    let pg_pool: PgPool = get_db_pool().clone();
    let sql = r#"
            SELECT schema_name name
            FROM information_schema.schemata
            WHERE schema_name NOT IN ('information_schema', 'pg_catalog', 'pg_toast')
            ORDER BY schema_name;
        "#;
    let data = sqlx::query_as::<_, Schema>(sql).fetch_all(&pg_pool).await?;
    Ok(data)
}

pub async fn query_tables(schema: String) -> AppResult<Vec<Table>> {
    let pg_pool: PgPool = get_db_pool().clone();
    let sql = r#"
        SELECT
            t.table_name as name,
            COALESCE(gc.f_geometry_column, '') as geometry
        FROM information_schema.tables t
        LEFT JOIN geometry_columns gc
            ON t.table_name = gc.f_table_name
            AND t.table_schema = gc.f_table_schema
        WHERE t.table_schema = $1
          AND t.table_type = 'BASE TABLE'
        ORDER BY t.table_name;
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
        SELECT column_name name, udt_name udt
        FROM information_schema.columns
        WHERE table_schema = $1 AND table_name = $2
        ORDER BY ordinal_position;
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

    let sql = r#"
        SELECT srid as name
        FROM geometry_columns
        WHERE f_table_schema = $1
          AND f_table_name = $2
          AND f_geometry_column = $3
    "#;

    let data = sqlx::query_as::<_, Srid>(sql)
        .bind(schema)
        .bind(table)
        .bind(geometry)
        .fetch_optional(&pg_pool)
        .await?;

    Ok(data.unwrap_or(Srid { name: 0 }))
}

pub async fn query_extent(layer: &Layer) -> AppResult<Extent> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql_estimate = r#"
        SELECT
            ST_XMin(box) as xmin, ST_YMin(box) as ymin,
            ST_XMax(box) as xmax, ST_YMax(box) as ymax
        FROM (
            SELECT ST_Transform(ST_SetSRID(ST_EstimatedExtent($1, $2, $3), $4), 4326) as box
        ) as sub
    "#;

    let estimate = sqlx::query_as::<_, Extent>(sql_estimate)
        .bind(&layer.schema)
        .bind(&layer.table_name)
        .bind(layer.get_geom())
        .fetch_optional(&pg_pool)
        .await;

    if let Ok(Some(ext)) = estimate
        && (ext.xmax != 0.0 || ext.xmin != 0.0)
    {
        return Ok(ext);
    }

    let geom_col = escape_identifier(&layer.get_geom());
    let schema_safe = escape_identifier(&layer.schema);
    let table_safe = escape_identifier(&layer.table_name);

    let sql_calc = format!(
        r#"
        SELECT
            COALESCE(ST_XMin(ST_Extent(ST_Transform({geom}, 4326))), -180) AS xmin,
            COALESCE(ST_YMin(ST_Extent(ST_Transform({geom}, 4326))), -90) AS ymin,
            COALESCE(ST_XMax(ST_Extent(ST_Transform({geom}, 4326))), 180) AS xmax,
            COALESCE(ST_YMax(ST_Extent(ST_Transform({geom}, 4326))), 90) AS ymax
        FROM {schema}.{table}
        "#,
        geom = geom_col,
        schema = schema_safe,
        table = table_safe
    );

    let extent = sqlx::query_as::<_, Extent>(&sql_calc)
        .fetch_one(&pg_pool)
        .await?;

    Ok(extent)
}
