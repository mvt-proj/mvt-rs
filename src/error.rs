use salvo::prelude::*;
// use std::io;
use crate::html::main::ErrorTemplate;
use askama::Template;
use std::num::TryFromIntError;
use thiserror::Error;
use::maplibre_legend::LegendError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Error parsing params: {0}")]
    RequestParamError(String),

    #[error("Error parsing request: `{0}`")]
    SerdeJSONError(#[from] serde_json::Error),

    #[error("Failed to generate template")]
    AskamaRenderError(#[from] askama::Error),

    #[error("Error parsing header: `{0}`")]
    ParseHeaderError(#[from] salvo::http::header::InvalidHeaderValue),

    #[error("Error executing SQL: `{0}`")]
    SQLError(#[from] sqlx::Error),

    #[error("Migrate error: `{0}`")]
    MigrateError(#[from] sqlx::migrate::MigrateError),

    #[error("Basic Authentication error: {0}")]
    BasicAuthError(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error("User not found: {0}")]
    UserNotFoundError(String),

    #[error("Cache not found: {0}")]
    CacheNotFount(String),

    #[error("JWT encoding error: {0}")]
    JwtEncodeError(#[from] jsonwebtoken::errors::Error),

    #[error("Failed to hash password: `{0}`")]
    PasswordHashError(#[from] argon2::password_hash::errors::Error),

    #[error("Redis error: `{0}`")]
    RedisError(String),

    #[error("Failed to create Redis connection manager")]
    ConnectionManager(#[from] redis::RedisError),

    #[error("Failed to build connection pool")]
    Pool(#[from] bb8::RunError<redis::RedisError>),

    #[error("Conversion error")]
    Conversion(#[from] TryFromIntError),

    #[error("Error initializing 'Auth': {0}")]
    AuthInitializationError(String),

    #[error("Error initializing 'Catalog': {0}")]
    CatalogInitializationError(String),

    #[error("Error creating file or reading directory: {0}")]
    FileCreationError(#[from] tokio::io::Error),

    #[error("Error reading directory or file: {0}")]
    NotFound(String),

    #[error("User not found")]
    UserNotFound,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Unauthorized access")]
    UnauthorizedAccess,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Timeout error")]
    TimeoutError,

    #[error("MapLibre legend error: {0}")]
    Legend(#[from] LegendError),
}

pub type AppResult<T> = Result<T, AppError>;

#[async_trait]
impl Writer for AppError {
    async fn write(mut self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        if let Some(status) = res.status_code {
            if status.as_u16() >= 400 && status.as_u16() <= 600 {
                let template = ErrorTemplate {
                    status: status.as_u16(),
                    message: self.to_string(),
                };

                res.render(Text::Html(template.render().unwrap()));
            }
        }
    }
}
