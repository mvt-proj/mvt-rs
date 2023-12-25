use salvo::prelude::*;
use serde::Serialize;
use sqlx::{FromRow, PgPool};

use crate::get_db_pool;

#[derive(FromRow, Serialize, Debug)]
struct Schema {
    name: String,
}

#[derive(FromRow, Serialize, Debug)]
struct Table {
    name: String,
    geometry: String,
}

#[derive(FromRow, Serialize, Debug)]
struct Field {
    name: String,
    udt: String,
}

#[derive(FromRow, Serialize, Debug)]
struct SRID {
    name: i32,
}

async fn query_schemas() -> Result<Vec<Schema>, sqlx::Error> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = format!(
        r#"
            SELECT schema_name name
            FROM information_schema.schemata
            ORDER BY schema_name;
        "#
    );

    let data = sqlx::query_as::<_, Schema>(&sql)
        .fetch_all(&pg_pool)
        .await?;
    Ok(data)
}

async fn query_tables(schema: String) -> Result<Vec<Table>, sqlx::Error> {
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

async fn query_fields(schema: String, table: String) -> Result<Vec<Field>, sqlx::Error> {
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

async fn query_srid(schema: String, table: String, geometry: String) -> Result<SRID, sqlx::Error> {
    let pg_pool: PgPool = get_db_pool().clone();

    let sql = format!(
        r#"
            SELECT Find_SRID('{schema}', '{table}', '{geometry}') AS name
            FROM {table}
            LIMIT 1;
        "#
    );
    dbg!(&sql);

    let data = sqlx::query_as::<_, SRID>(&sql).fetch_one(&pg_pool).await?;
    Ok(data)
}

#[handler]
pub async fn schemas() -> Result<Json<Vec<Schema>>, StatusError> {
    let rv = query_schemas().await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {:?}",
                    e
                )))
        }
    }
}

#[handler]
pub async fn tables(req: &mut Request) -> Result<Json<Vec<Table>>, StatusError> {
    let schema = req.param::<String>("schema").unwrap();
    let rv = query_tables(schema).await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {:?}",
                    e
                )))
        }
    }
}

#[handler]
pub async fn fields(req: &mut Request) -> Result<Json<Vec<Field>>, StatusError> {
    let schema = req.param::<String>("schema").unwrap();
    let table = req.param::<String>("table").unwrap();
    let rv = query_fields(schema, table).await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {:?}",
                    e
                )))
        }
    }
}

#[handler]
pub async fn srid(req: &mut Request) -> Result<Json<SRID>, StatusError> {
    let schema = req.param::<String>("schema").unwrap();
    let table = req.param::<String>("table").unwrap();
    let geometry = req.param::<String>("geometry").unwrap();
    let rv = query_srid(schema, table, geometry).await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {:?}",
                    e
                )))
        }
    }
}
