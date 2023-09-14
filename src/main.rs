use askama::Template;
use clap::{Arg, Command};
use dotenv;
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

#[derive(Clone)]
pub struct Config {
    pub db_pool: PgPool,
    pub layers_config: LayersConfig,
    pub disk_cache: cache::DiskCache,
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
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL needs to be defined");
    let db_pool_size_min = std::env::var("POOLSIZEMIN").unwrap_or("2".to_string());
    let db_pool_size_max = std::env::var("POOLSIZEMAX").unwrap_or("5".to_string());
    let delete_cache = std::env::var("DELETECACHE").unwrap_or("0".to_string());

    let db_pool_size_min: u32 = db_pool_size_min.parse().unwrap();
    let db_pool_size_max: u32 = db_pool_size_max.parse().unwrap();
    let delete_cache: u8 = delete_cache.parse().unwrap();


    let matches = Command::new("mvt-rs vector tiles server")
        .arg(Arg::new("layers")
            .short('l')
            .long("layers")
            .value_name("LAYERS")
            .default_value("layers")
            .help("Directory where the layer configuration files are placed")
        )
        .arg(Arg::new("cachedir")
            .short('c')
            .long("cachedir")
            .value_name("CACHEDIR")
            .default_value("cache")
            .help("Directory where cache files are placed")
        )
        .get_matches();

    let layers_dir = matches.get_one::<String>("layers").expect("required");
    let cache_dir = matches.get_one::<String>("cachedir").expect("required");

    let layers_config = LayersConfig::new(layers_dir).await.expect(
        "You must have a layers directory to place the layer files to be served.",
        );

    let disk_cache = cache::DiskCache::new(cache_dir.into());
    if delete_cache != 0 {
        disk_cache.delete_cache_dir(layers_config.clone()).await;
    }

    let db_pool = match make_db_pool(&db_url, db_pool_size_min, db_pool_size_max).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Could not connect to the database {}", &db_url);
            panic!("Database connection error: {}", e);
        }
    };

    let config = Config {
        db_pool,
        layers_config,
        disk_cache,
    };

    let acceptor = TcpListener::new(format!("{host}:{port}")).bind().await;
    Server::new(acceptor)
        .serve(routes::app_router(config.clone()))
        .await;
}
