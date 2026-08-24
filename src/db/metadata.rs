use crate::{
    error::{AppError, AppResult},
    get_db_registry,
    models::catalog::Layer,
};
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
pub struct FieldWithComment {
    pub name: String,
    pub udt: String,
    pub description: Option<String>,
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

#[derive(FromRow, Serialize, Debug)]
pub struct SpatialIndex {
    pub has_index: bool,
    pub is_view: bool,
}

fn escape_identifier(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

pub async fn query_schemas(database_id: &str) -> AppResult<Vec<Schema>> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();
    let sql = r#"
            SELECT schema_name name
            FROM information_schema.schemata
            WHERE schema_name NOT IN ('information_schema', 'pg_catalog', 'pg_toast')
            ORDER BY schema_name;
        "#;
    let data = sqlx::query_as::<_, Schema>(sql).fetch_all(&pg_pool).await?;
    Ok(data)
}

pub async fn query_tables(database_id: &str, schema: String) -> AppResult<Vec<Table>> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

    let sql = r#"
        SELECT
            c.relname as name,
            gc.f_geometry_column as geometry
        FROM pg_class c
        JOIN pg_namespace n
            ON n.oid = c.relnamespace
        JOIN geometry_columns gc
            ON c.relname = gc.f_table_name
            AND n.nspname = gc.f_table_schema
        WHERE n.nspname = $1
          AND c.relkind IN ('r', 'p', 'v', 'm')
        ORDER BY c.relname;
    "#;

    let data = sqlx::query_as::<_, Table>(sql)
        .bind(schema)
        .fetch_all(&pg_pool)
        .await?;

    Ok(data)
}

pub async fn query_fields(
    database_id: &str,
    schema: String,
    table: String,
) -> AppResult<Vec<Field>> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

    let sql = r#"
        SELECT
            a.attname AS name,
            t.typname AS udt
        FROM pg_attribute a
        JOIN pg_class c      ON a.attrelid = c.oid
        JOIN pg_namespace n  ON c.relnamespace = n.oid
        JOIN pg_type t       ON a.atttypid = t.oid
        WHERE n.nspname = $1
          AND c.relname = $2
          AND a.attnum > 0
          AND NOT a.attisdropped
        ORDER BY a.attnum;
    "#;

    let data = sqlx::query_as::<_, Field>(sql)
        .bind(schema)
        .bind(table)
        .fetch_all(&pg_pool)
        .await?;

    Ok(data)
}

pub async fn query_fields_with_comments(
    database_id: &str,
    schema: String,
    table: String,
) -> AppResult<Vec<FieldWithComment>> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

    let sql = r#"
        SELECT
            a.attname AS name,
            t.typname AS udt,
            col_description(c.oid, a.attnum) AS description
        FROM pg_attribute a
        JOIN pg_class c      ON a.attrelid = c.oid
        JOIN pg_namespace n  ON c.relnamespace = n.oid
        JOIN pg_type t       ON a.atttypid = t.oid
        WHERE n.nspname = $1
          AND c.relname = $2
          AND a.attnum > 0
          AND NOT a.attisdropped
        ORDER BY a.attnum;
    "#;

    let data = sqlx::query_as::<_, FieldWithComment>(sql)
        .bind(schema)
        .bind(table)
        .fetch_all(&pg_pool)
        .await?;

    Ok(data)
}

pub async fn query_srid(
    database_id: &str,
    schema: String,
    table: String,
    geometry: String,
) -> AppResult<Srid> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

    let sql = r#"
        SELECT srid as name
        FROM geometry_columns
        WHERE f_table_schema = $1
          AND f_table_name = $2
          AND f_geometry_column = $3
    "#;

    let data: Option<Srid> = sqlx::query_as::<_, Srid>(sql)
        .bind(schema)
        .bind(table)
        .bind(geometry)
        .fetch_optional(&pg_pool)
        .await?;

    Ok(data.unwrap_or(Srid { name: 0 }))
}

pub async fn query_extent(layer: &Layer) -> AppResult<Extent> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(&layer.database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

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

    let extent = sqlx::query_as::<_, Extent>(sqlx::AssertSqlSafe(sql_calc))
        .fetch_one(&pg_pool)
        .await?;

    Ok(extent)
}

/// Checks whether `geometry` on `schema.table` is covered by a spatial index
/// (GiST, SP-GiST or BRIN). Advisory only: if the relation can't be found in
/// the catalog, defaults to `has_index: true` so a lookup edge case doesn't
/// surface a misleading warning.
pub async fn query_has_spatial_index(
    database_id: &str,
    schema: String,
    table: String,
    geometry: String,
) -> AppResult<SpatialIndex> {
    let pg_pool: PgPool = get_db_registry()
        .get_pool(database_id)
        .ok_or(AppError::DatabaseError("DB not found".to_string()))?
        .clone();

    let sql = r#"
        SELECT
            (c.relkind = 'v') AS is_view,
            EXISTS (
                SELECT 1
                FROM pg_index i
                JOIN pg_class ic ON ic.oid = i.indexrelid
                JOIN pg_am am ON am.oid = ic.relam
                JOIN pg_attribute a
                    ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
                WHERE i.indrelid = c.oid
                  AND a.attname = $3
                  AND am.amname IN ('gist', 'spgist', 'brin')
            ) AS has_index
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = $1
          AND c.relname = $2
    "#;

    let data: Option<SpatialIndex> = sqlx::query_as::<_, SpatialIndex>(sql)
        .bind(schema)
        .bind(table)
        .bind(geometry)
        .fetch_optional(&pg_pool)
        .await?;

    Ok(data.unwrap_or(SpatialIndex {
        has_index: true,
        is_view: false,
    }))
}

/// `CREATE INDEX` suggestion for a missing spatial index, with identifiers
/// escaped the same way as the rest of this module.
pub fn suggested_spatial_index_sql(schema: &str, table: &str, geometry: &str) -> String {
    format!(
        "CREATE INDEX {} ON {}.{} USING GIST ({});",
        escape_identifier(&format!("idx_{table}_{geometry}")),
        escape_identifier(schema),
        escape_identifier(table),
        escape_identifier(geometry),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_spatial_index_sql_escapes_identifiers() {
        assert_eq!(
            suggested_spatial_index_sql("public", "parcels", "geom"),
            r#"CREATE INDEX "idx_parcels_geom" ON "public"."parcels" USING GIST ("geom");"#
        );
    }

    #[test]
    fn suggested_spatial_index_sql_escapes_quotes_in_identifiers() {
        assert_eq!(
            suggested_spatial_index_sql("public", r#"weird"table"#, "geom"),
            "CREATE INDEX \"idx_weird\"\"table_geom\" ON \"public\".\"weird\"\"table\" USING GIST (\"geom\");"
        );
    }
}
