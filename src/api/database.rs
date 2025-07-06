use salvo::prelude::*;

use crate::database::{
    Field, Schema, Srid, Table, query_fields, query_schemas, query_srid, query_tables,
};

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
                    "An error occurred while retrieving the data. {e:?}"
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
                    "An error occurred while retrieving the data. {e:?}"
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
                    "An error occurred while retrieving the data. {e:?}"
                )))
        }
    }
}

#[handler]
pub async fn srid(req: &mut Request) -> Result<Json<Srid>, StatusError> {
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
                    "An error occurred while retrieving the data. {e:?}"
                )))
        }
    }
}
