use salvo::prelude::*;

use crate::db::metadata::{
    Field, Schema, Srid, Table, query_fields, query_schemas, query_srid, query_tables,
};

#[handler]
pub async fn schemas(req: &mut Request) -> Result<Json<Vec<Schema>>, StatusError> {
    let db_id = req.query::<String>("database_id").unwrap_or_else(|| "default".to_string());
    let rv = query_schemas(&db_id).await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {e:?}"
                )))
        }
    }
}

#[handler]
pub async fn tables(req: &mut Request) -> Result<Json<Vec<Table>>, StatusError> {
    let db_id = req.query::<String>("database_id").unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let rv = query_tables(&db_id, schema).await;

    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {e:?}"
                )))
        }
    }
}

#[handler]
pub async fn fields(req: &mut Request) -> Result<Json<Vec<Field>>, StatusError> {
    let db_id = req.query::<String>("database_id").unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    let rv = query_fields(&db_id, schema, table).await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {e:?}"
                )))
        }
    }
}

#[handler]
pub async fn srid(req: &mut Request) -> Result<Json<Srid>, StatusError> {
    let db_id = req.query::<String>("database_id").unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    let geometry = req.query::<String>("geometry").unwrap_or_default();
    let rv = query_srid(&db_id, schema, table, geometry).await;
    match rv {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            tracing::error!("{}", e);
            Err(StatusError::bad_request()
                .brief("An error occurred while retrieving the data.")
                .cause(format!(
                    "An error occurred while retrieving the data. {e:?}"
                )))
        }
    }
}
