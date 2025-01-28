// use anyhow::Ok;
use askama::Template;
use salvo::prelude::*;

use crate::{
    database::{
        query_fields, query_schemas, query_srid, query_tables, Field, Schema,
        Srid, Table,
    },
    error::{AppError, AppResult},
};

#[derive(Template)]
#[template(path = "admin/database/schemas.html")]
struct SchemaTemplate<'a> {
    schemas: &'a Vec<Schema>,
    schema_selected: String,
    table_selected: String,
}

#[derive(Template)]
#[template(path = "admin/database/tables.html")]
struct TableTemplate<'a> {
    tables: &'a Vec<Table>,
    table_selected: String,
}

#[derive(Template)]
#[template(path = "admin/database/fields.html")]
struct FieldTemplate<'a> {
    fields: &'a Vec<Field>,
    fields_selected: Vec<String>,
}

#[derive(Template)]
#[template(path = "admin/database/srid.html")]
struct SRIDTemplate<'a> {
    srid: &'a Srid,
}

#[handler]
pub async fn schemas(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let schema_selected = req.query::<String>("schema_selected").unwrap_or_default();
    let table_selected = req.query::<String>("table_selected").unwrap_or_default();
    let rv = query_schemas().await?;

    let template = SchemaTemplate {
        schemas: &rv,
        schema_selected,
        table_selected,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn tables(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let schema = req
        .query::<String>("schema")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    let table_selected = req.query::<String>("table_selected").unwrap_or_default();

    let rv = query_tables(schema).await?;
    let template = TableTemplate {
        tables: &rv,
        table_selected,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn fields(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let schema = req
        .query::<String>("schema")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    let table = req
        .query::<String>("table")
        .ok_or(AppError::RequestParamError("table".to_string()))?;
    let fields_selected_vec = req
        .query::<Vec<String>>("fields_selected")
        .unwrap_or_default();

    let fields_selected: Vec<String> = fields_selected_vec
        .first()
        .map(|fields_selected_str| {
            fields_selected_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    let rv = query_fields(schema, table).await?;

    let template = FieldTemplate {
        fields: &rv,
        fields_selected,
    };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}

#[handler]
pub async fn srid(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let schema = req
        .query::<String>("schema")
        .ok_or(AppError::RequestParamError("schema".to_string()))?;
    let table = req
        .query::<String>("table")
        .ok_or(AppError::RequestParamError("table".to_string()))?;
    let geometry = req
        .query::<String>("geometry")
        .ok_or(AppError::RequestParamError("geometry".to_string()))?;
    let rv = query_srid(schema, table, geometry).await?;

    let template = SRIDTemplate { srid: &rv };
    let html_render = template.render()?;
    res.render(Text::Html(html_render));
    Ok(())
}
