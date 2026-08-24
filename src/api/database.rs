use salvo::prelude::*;

use crate::db::metadata::{
    Field, Schema, SpatialIndex, Srid, Table, query_fields, query_has_spatial_index,
    query_schemas, query_srid, query_tables,
};
use crate::error::AppResult;

#[handler]
pub async fn schemas(req: &mut Request) -> AppResult<Json<Vec<Schema>>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    Ok(Json(query_schemas(&db_id).await?))
}

#[handler]
pub async fn tables(req: &mut Request) -> AppResult<Json<Vec<Table>>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    Ok(Json(query_tables(&db_id, schema).await?))
}

#[handler]
pub async fn fields(req: &mut Request) -> AppResult<Json<Vec<Field>>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    Ok(Json(query_fields(&db_id, schema, table).await?))
}

#[handler]
pub async fn srid(req: &mut Request) -> AppResult<Json<Srid>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    let geometry = req.query::<String>("geometry").unwrap_or_default();
    Ok(Json(query_srid(&db_id, schema, table, geometry).await?))
}

#[handler]
pub async fn spatial_index(req: &mut Request) -> AppResult<Json<SpatialIndex>> {
    let db_id = req
        .query::<String>("database_id")
        .unwrap_or_else(|| "default".to_string());
    let schema = req.query::<String>("schema").unwrap_or_default();
    let table = req.query::<String>("table").unwrap_or_default();
    let geometry = req.query::<String>("geometry").unwrap_or_default();
    Ok(Json(
        query_has_spatial_index(&db_id, schema, table, geometry).await?,
    ))
}
