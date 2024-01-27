use askama::Template;
use salvo::prelude::*;

use crate::database::{Schema, Table, Field, SRID, query_schemas, query_tables, query_fields, query_srid};

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
    srid: &'a SRID,
}


#[handler]
pub async fn schemas(req: &mut Request, res: &mut Response) {
    let schema_selected =  req.query::<String>("schema_selected").unwrap_or(String::new());
    let table_selected =  req.query::<String>("table_selected").unwrap_or(String::new());
    let rv = query_schemas().await.unwrap();

    let template = SchemaTemplate {
        schemas: &rv,
        schema_selected,
        table_selected,
    };
    res.render(Text::Html(template.render().unwrap()));

}

#[handler]
pub async fn tables(req: &mut Request, res: &mut Response) {
    let schema = req.query::<String>("schema").unwrap();
    let table_selected =  req.query::<String>("table_selected").unwrap_or(String::new());

    let rv = query_tables(schema).await.unwrap();
    let template = TableTemplate {
        tables: &rv,
        table_selected,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn fields(req: &mut Request, res: &mut Response) {
    let schema = req.query::<String>("schema").unwrap();
    let table = req.query::<String>("table").unwrap();
    let fields_selected_vec =  req.query::<Vec<String>>("fields_selected").unwrap_or(vec![]);

    let fields_selected: Vec<String> = fields_selected_vec
        .get(0)
        .map(|fields_selected_str| {
            fields_selected_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        })
        .unwrap_or_else(|| Vec::new());

    let rv = query_fields(schema, table).await.unwrap();

    let template = FieldTemplate {
        fields: &rv,
        fields_selected,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
pub async fn srid(req: &mut Request, res: &mut Response) {
    let schema = req.query::<String>("schema").unwrap();
    let table = req.query::<String>("table").unwrap();
    let geometry = req.query::<String>("geometry").unwrap();
    let rv = query_srid(schema, table, geometry).await.unwrap();

    let template = SRIDTemplate {
        srid: &rv,
    };
    res.render(Text::Html(template.render().unwrap()));
}
