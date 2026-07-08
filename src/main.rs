use config::{categories::get_categories as get_cf_categories, settings::Settings};
use salvo::prelude::*;
use salvo::server::ServerHandle;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::signal;
use tokio::sync::{OnceCell, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod auth;
mod cache;
mod cluster;
mod config;
mod db;
mod error;
mod filters;
mod html;
mod i18n;
mod models;
mod monitor;
mod plugins;
mod routes;
mod services;

use crate::db::connection::DbRegistry;
use crate::error::AppResult;
use auth::Auth;
use cache::cachewrapper::CacheWrapper;
use models::{catalog::Catalog, category::Category, styles::Style};
use monitor::start_system_monitor;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static DB_REGISTRY: OnceLock<DbRegistry> = OnceLock::new();
#[inline]
pub fn get_db_registry() -> &'static DbRegistry {
    DB_REGISTRY.get().unwrap()
}

static SQLITE_CONF: OnceLock<SqlitePool> = OnceLock::new();
#[inline]
pub fn get_cf_pool() -> &'static SqlitePool {
    SQLITE_CONF.get().unwrap()
}

static MAP_ASSETS_DIR: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_map_assets() -> &'static String {
    MAP_ASSETS_DIR.get().unwrap()
}

static JWT_SECRET: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_jwt_secret() -> &'static String {
    JWT_SECRET.get().unwrap()
}

static CLUSTER_SECRET: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_cluster_secret() -> &'static str {
    CLUSTER_SECRET.get().map(|s| s.as_str()).unwrap_or("")
}

static CONFIG_DIR: OnceLock<String> = OnceLock::new();
#[inline]
pub fn get_config_dir() -> &'static str {
    CONFIG_DIR.get().map(|s| s.as_str()).unwrap_or("")
}

static PUBLIC_URL: OnceLock<Option<String>> = OnceLock::new();
#[inline]
pub fn get_public_url() -> Option<&'static str> {
    PUBLIC_URL.get().and_then(|url| url.as_deref())
}

/// Delay applied before invalidating the shared cache after a layer edit.
/// `Some` in clustered owner/shared modes (so peers reload the new config
/// before the cache is cleared); `None` means invalidate immediately.
static CACHE_INVALIDATION_DELAY: OnceLock<Option<Duration>> = OnceLock::new();
#[inline]
pub fn get_cache_invalidation_delay() -> Option<Duration> {
    CACHE_INVALIDATION_DELAY.get().copied().flatten()
}

static CACHE_WRAPPER: OnceLock<CacheWrapper> = OnceLock::new();
#[inline]
pub fn get_cache_wrapper() -> &'static CacheWrapper {
    CACHE_WRAPPER.get().unwrap()
}

static PLUGIN_REGISTRY: OnceLock<plugins::LuaPluginRegistry> = OnceLock::new();
#[inline]
pub fn get_plugin_registry() -> &'static plugins::LuaPluginRegistry {
    PLUGIN_REGISTRY.get().unwrap()
}

static CATALOG: OnceCell<RwLock<Catalog>> = OnceCell::const_new();
#[inline]
pub async fn get_catalog() -> &'static RwLock<Catalog> {
    CATALOG.get().unwrap()
}

static CATEGORIES: OnceCell<RwLock<Vec<Category>>> = OnceCell::const_new();
#[inline]
pub async fn get_categories() -> &'static RwLock<Vec<Category>> {
    CATEGORIES.get().unwrap()
}

static AUTH: OnceCell<RwLock<Auth>> = OnceCell::const_new();
#[inline]
pub async fn get_auth() -> &'static RwLock<Auth> {
    AUTH.get().unwrap()
}

static STYLES: OnceCell<RwLock<Vec<Style>>> = OnceCell::const_new();
#[inline]
pub async fn get_styles_cache() -> &'static RwLock<Vec<Style>> {
    STYLES.get().unwrap()
}

pub async fn reload_styles_cache() -> AppResult<()> {
    let styles = config::styles::get_styles(Some(get_cf_pool())).await?;
    *get_styles_cache().await.write().await = styles;
    Ok(())
}

async fn initialize_auth(config_dir: &str, pool: &SqlitePool) -> AppResult<Auth> {
    let auth = Auth::new(config_dir, pool).await?;
    Ok(auth)
}

async fn initialize_catalog(pool: &SqlitePool) -> AppResult<Catalog> {
    let catalog = Catalog::new(pool).await?;
    Ok(catalog)
}

async fn listen_shutdown_signal(handle: ServerHandle) {
    // Wait Shutdown Signal
    let ctrl_c = async {
        // Handle Ctrl+C signal
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        // Handle SIGTERM on Unix systems
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(windows)]
    let terminate = async {
        // Handle Ctrl+C on Windows (alternative implementation)
        signal::windows::ctrl_c()
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    // Wait for either signal to be received
    tokio::select! {
        _ = ctrl_c => println!("ctrl_c signal received"),
        _ = terminate => println!("terminate signal received"),
    };

    // Graceful Shutdown Server
    handle.stop_graceful(None);
}

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mvt_server=info,warn".into()),
        )
        .init();

    tokio::spawn(async {
        start_system_monitor().await;
    });

    let settings = Settings::new()
        .map_err(|e| crate::error::AppError::ConfigurationError(e.to_string()))?;

    // Strict validation
    if settings.security.session_secret.len() < 32 {
        tracing::error!("CRITICAL: Session secret must be at least 32 bytes. Found: {}", settings.security.session_secret.len());
        std::process::exit(1);
    }

    if let Err(e) = settings.validate() {
        tracing::error!("CRITICAL: {}", e);
        std::process::exit(1);
    }

    CONFIG_DIR.set(settings.paths.config.clone()).unwrap();
    PUBLIC_URL.set(settings.server.public_url.clone()).unwrap();

    // In clustered owner/shared modes, defer cache invalidation so every peer
    // reloads the edited config (within its watch interval) before the shared
    // cache is cleared; otherwise a lagging peer could repopulate it with a
    // stale tile. Standalone/client clear immediately (client has no admin).
    let cache_invalidation_delay = match settings.cluster.mode.as_str() {
        "owner" | "shared" => Some(Duration::from_secs(
            settings.cluster.config_watch_interval_secs
                + settings.cluster.cache_invalidation_extra_delay_secs,
        )),
        _ => None,
    };
    CACHE_INVALIDATION_DELAY.set(cache_invalidation_delay).unwrap();

    if settings.cluster.mode == "client" {
        let owner_url = settings.cluster.owner_url.clone().expect("client requires owner_url");
        let secret = settings.cluster.shared_secret.clone().expect("client requires shared_secret");
        CLUSTER_SECRET.set(secret.clone()).unwrap();

        let snapshot = cluster::bootstrap::bootstrap_from_owner(
            &owner_url,
            &secret,
            Duration::from_secs(settings.cluster.config_watch_interval_secs),
        )
        .await;

        // Initialize globals that do not depend on the config SQLite.
        let db_registry = DbRegistry::new(
            &settings.postgres_databases.connections,
            settings.postgres_databases.pool_min,
            settings.postgres_databases.pool_max,
        )
        .await?;
        let cache_wrapper = CacheWrapper::initialize_cache(
            settings.database.redis_url.clone(),
            settings.paths.cache.clone().into(),
            snapshot.catalog.clone(),
        )
        .await?;
        let plugin_registry = plugins::LuaPluginRegistry::new(&settings.paths.plugins);

        DB_REGISTRY.set(db_registry).unwrap();
        MAP_ASSETS_DIR.set(settings.paths.assets.clone()).unwrap();
        JWT_SECRET.set(settings.security.jwt_secret.clone()).unwrap();
        CACHE_WRAPPER.set(cache_wrapper).unwrap();
        PLUGIN_REGISTRY.set(plugin_registry).unwrap();

        // Initialize the four in-memory states from the snapshot.
        CATALOG.set(RwLock::new(snapshot.catalog.clone())).unwrap();
        CATEGORIES.set(RwLock::new(snapshot.categories.clone())).unwrap();
        STYLES.set(RwLock::new(snapshot.styles.clone())).unwrap();
        {
            let mut auth = snapshot.auth.clone();
            auth.config_dir = settings.paths.config.clone();
            AUTH.set(RwLock::new(auth)).unwrap();
        }

        cluster::watcher::start_config_watcher(
            cluster::backend::SyncBackend::remote(owner_url, secret),
            Duration::from_secs(settings.cluster.config_watch_interval_secs),
            settings.paths.config.clone(),
        );
    } else {
        // Existing standalone/shared/owner init path.
        let config_path = Path::new(&settings.paths.config).join(&settings.database.sqlite_path);
        let db_conn = config_path.to_str().expect("Invalid configuration path");
        let cf_pool = config::db::init_sqlite(db_conn).await?;

        let auth = initialize_auth(&settings.paths.config, &cf_pool).await?;
        let catalog = initialize_catalog(&cf_pool).await?;

        let db_registry = DbRegistry::new(
            &settings.postgres_databases.connections,
            settings.postgres_databases.pool_min,
            settings.postgres_databases.pool_max,
        )
        .await?;

        let categories = get_cf_categories(Some(&cf_pool)).await?;
        let styles = config::styles::get_styles(Some(&cf_pool)).await?;
        let cache_wrapper = CacheWrapper::initialize_cache(
            settings.database.redis_url.clone(),
            settings.paths.cache.clone().into(),
            catalog.clone(),
        )
        .await?;

        let plugin_registry = plugins::LuaPluginRegistry::new(&settings.paths.plugins);

        DB_REGISTRY.set(db_registry).unwrap();
        SQLITE_CONF.set(cf_pool).unwrap();
        MAP_ASSETS_DIR
            .set(settings.paths.assets.clone())
            .unwrap();
        JWT_SECRET.set(settings.security.jwt_secret.clone()).unwrap();
        if let Some(secret) = settings.cluster.shared_secret.clone() {
            CLUSTER_SECRET.set(secret).unwrap();
        }
        CACHE_WRAPPER.set(cache_wrapper).unwrap();
        PLUGIN_REGISTRY.set(plugin_registry).unwrap();
        CATALOG.set(RwLock::new(catalog)).unwrap();
        CATEGORIES.set(RwLock::new(categories)).unwrap();
        AUTH.set(RwLock::new(auth)).unwrap();
        STYLES.set(RwLock::new(styles)).unwrap();

        if settings.cluster.mode == "shared" {
            cluster::watcher::start_config_watcher(
                cluster::backend::SyncBackend::Local { pool: get_cf_pool() },
                Duration::from_secs(settings.cluster.config_watch_interval_secs),
                settings.paths.config.clone(),
            );
        }
    }

    let i18n_service = Arc::new(i18n::I18n::new());

    let acceptor = TcpListener::new(format!("{}:{}", settings.server.host, settings.server.port))
        .bind()
        .await;
    let server = Server::new(acceptor);
    let handle = server.handle();

    tokio::spawn(listen_shutdown_signal(handle));

    // Note: routes need to be updated to use Settings instead of AppConfig
    server
        .serve(routes::app_router(&settings, i18n_service))
        .await;

    Ok(())
}
