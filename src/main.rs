use askama::Template;
use dotenv;
use once_cell::sync::OnceCell;
use salvo::prelude::*;
use sqlx::PgPool;

mod cache;
mod config;
mod db;
mod routes;
mod tiles;
mod health;
use config::LayersConfig;
use db::make_db_pool;

// pub const CACHE_DIR: &str = "./cache";
pub static CACHE_DIR: OnceCell<&str> = OnceCell::new();

#[derive(Clone)]
pub struct Config {
    pub db_pool: PgPool,
    pub layers_config: LayersConfig,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    layers_config: &'a LayersConfig,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    name: &'a str,
    alias: &'a str,
    geometry: &'a str,
}


#[handler]
async fn index(depot: &mut Depot, res: &mut Response) {
    let config = depot.obtain::<Config>().unwrap();
    let config = config.clone();
    let layers_config: LayersConfig = config.layers_config;

    let template = IndexTemplate {
        layers_config: &layers_config,
    };
    res.render(Text::Html(template.render().unwrap()));
}

#[handler]
async fn mapview(req: &mut Request, res: &mut Response, depot: &mut Depot) {

    let config = depot.obtain::<Config>().unwrap();
    let config = config.clone();
    let layers_config: LayersConfig = config.layers_config;

    let layer_name = req.param::<String>("layer").unwrap();
    let layer = layers_config.find_layer_by_name(&layer_name).unwrap();
    let geometry = match layer.geometry.as_str() {
        "points" => "circle",
        "lines" => "line",
        "polygons" => "fill",
        _ => &layer.geometry,
    };

    let template = MapTemplate {
        name: &layer.name,
        alias: &layer.alias,
        geometry
    };
    res.render(Text::Html(template.render().unwrap()));
}


// #[handler]
// async fn index(depot: &mut Depot, res: &mut Response) {
//     let config = depot.obtain::<Config>().unwrap();
//     let config = config.clone();
//     let layers_config: LayersConfig = config.layers_config;
//
//     let hello_tmpl = IndexTemplate {
//         layers_config: &layers_config,
//     };
//     res.render(Text::Html(hello_tmpl.render().unwrap()));
// }


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        // .json()
        .with_env_filter("error")
        .with_env_filter("warn")
        // .with_env_filter("info")
        .init();

    dotenv::dotenv().ok();
    let host = std::env::var("IPHOST").unwrap_or("127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or("5887".to_string());
    let db_url = std::env::var("DATABASE_URL").expect("Falta definir DATABASE_URL");
    let db_pool_size_min = std::env::var("POOLSIZEMIN").unwrap_or("2".to_string());
    let db_pool_size_max = std::env::var("POOLSIZEMAX").unwrap_or("5".to_string());
    // let cache_dir = std::env::var("CACHE_DIR").expect("Falta definir directorio cache");
    let delete_cache = std::env::var("DELETECACHE").unwrap_or("0".to_string());

    let db_pool_size_min: u32 = db_pool_size_min.parse().unwrap();
    let db_pool_size_max: u32 = db_pool_size_max.parse().unwrap();
    let delete_cache: u8 = delete_cache.parse().unwrap();

    CACHE_DIR.set("./cache").unwrap();

    // CACHE_DIR.set(cache_dir).unwrap();

    let layers_config = LayersConfig::new().await.expect(
        "Debe tener un directorio de layers para colocar los archivos de las capas a publicar",
    );

    if delete_cache != 0 {
        cache::delete_cache_dir(CACHE_DIR.get().unwrap(), layers_config.clone()).await;
    }

    let db_pool = match make_db_pool(&db_url, db_pool_size_min, db_pool_size_max).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("No se pudo conectar a la base de datos db_pool_serv");
            panic!("Error base de datos: {}", e);
        }
    };

    let config = Config {
        db_pool,
        layers_config,
    };

    let acceptor = TcpListener::new(format!("{host}:{port}")).bind().await;
    Server::new(acceptor)
        .serve(routes::app_router(config.clone()))
        .await;
}
