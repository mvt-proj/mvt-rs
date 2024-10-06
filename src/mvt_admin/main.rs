use salvo::prelude::*;
use std::cell::OnceCell;
use sqlx::sqlite::SqlitePool;

mod args;
mod db;

use mvtrs::common::error::AppResult;

#[derive(Debug)]
pub struct AppState {
    db_pool: SqlitePool,
    // catalog: Catalog,
    // auth: Auth,
    // jwt_secret: String,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

pub fn get_app_state() -> &'static mut AppState {
    unsafe { APP_STATE.get_mut().unwrap() }
}

pub fn get_db_pool() -> &'static SqlitePool {
    unsafe { &APP_STATE.get().unwrap().db_pool }
}

// pub fn get_auth() -> &'static Auth {
//     unsafe { &APP_STATE.get().unwrap().auth }
// }
//
// pub fn get_jwt_secret() -> &'static String {
//     unsafe { &APP_STATE.get().unwrap().jwt_secret }
// }


#[handler]
async fn hello() -> &'static str {
    "Hello World"
}
#[handler]
async fn hello_zh() -> Result<&'static str, ()> {
    Ok("你好，世界！")
}

#[handler]
async fn get_layers(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para obtener la lista de capas
}

#[handler]
async fn create_layer(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para crear una nueva capa
}

#[handler]
async fn update_layer(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para actualizar una capa
}

#[handler]
async fn delete_layer(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para eliminar una capa
}

#[handler]
async fn get_groups(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para obtener la lista de grupos
}

#[handler]
async fn create_group(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para crear un nuevo grupo
}

#[handler]
async fn update_group(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para actualizar un grupo
}

#[handler]
async fn delete_group(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para eliminar un grupo
}

#[handler]
async fn get_users(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para obtener la lista de usuarios
}

#[handler]
async fn create_user(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para crear un nuevo usuario
}

#[handler]
async fn update_user(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para actualizar un usuario
}

#[handler]
async fn delete_user(req: &mut Request, res: &mut Response) {
    // Implementa la lógica para eliminar un usuario
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let app_config = args::parse_args().await?;

    dbg!(&app_config);

    println!("{}", app_config.salt_string);

    tracing_subscriber::fmt().init();

    let db_conn = "sqlite:mvtrs.db";
    let db_pool = db::make_db_pool(db_conn).await?;

    let app_state = AppState {
        db_pool,
        // catalog,
        // disk_cache,
        // auth,
        // jwt_secret: app_config.jwt_secret,
        // use_redis_cache,
        // redis_cache,
    };

    unsafe {
        APP_STATE.set(app_state).unwrap();
    }

    let acceptor = TcpListener::new(format!("{}:{}", app_config.hostadmin, app_config.portadmin)).bind().await;
    let router = Router::new()
        .get(hello)
        .push(Router::with_path("你好").get(hello_zh));
    println!("{:?}", router);
    Server::new(acceptor).serve(router).await;
    Ok(())
}
