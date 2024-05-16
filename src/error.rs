use salvo::prelude::*;
// use tokio::sync::mpsc::error;
use std::io;
use std::num::TryFromIntError;
use thiserror::Error;



#[derive(Error, Debug)]
pub enum AppError {
    #[error("io: `{0}`")]
    Io(#[from] io::Error),

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

    #[error("Basic Authentication error: {0}")]
    BasicAuthError(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error("User not found: {0}")]
    UserNotFoundError(String),

    #[error("Cache noty found: {0}")]
    CacheNotFount(String),

    #[error("JWT encoding error: {0}")]
    JwtEncodeError(#[from] jsonwebtoken::errors::Error),

    #[error("Failed to hash password: `{0}`")]
    PasswordHashError(#[from] argon2::password_hash::errors::Error),

    #[error("Redis error: `{0}`")]
    RedisError(String),

    #[error("failed to create Redis connection manager")]
    ConnectionManager(#[from] redis::RedisError),

    #[error("failed to build connection pool")]
    Pool(#[from] bb8::RunError<redis::RedisError>),

    #[error("Conversion error")]
    Conversion(#[from] TryFromIntError),

    // #[error("Error obtaining connection: {0}")]
    // ConnectionError(#[from] redis::RedisError),
}

pub type AppResult<T> = Result<T, AppError>;

#[async_trait]
impl Writer for AppError {
    async fn write(mut self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        res.render(Text::Plain(self.to_string()));
    }
}
