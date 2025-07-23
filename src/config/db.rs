use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use sqlx::{SqlitePool, migrate::Migrator};
use std::path::Path;

use crate::error::AppResult;
use std::fs;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!();

pub async fn init_sqlite(db_path: &str) -> AppResult<SqlitePool> {
    let db_url = format!("sqlite:{db_path}");

    if !Path::new(db_path).exists() {
        println!("Database file not found, initializing: {db_path}");

        if let Some(parent) = Path::new(db_path).parent() {
            fs::create_dir_all(parent).expect("Failed to create database directory");
            fs::File::create(db_path).expect("Failed to create database file");
        }
    }

    let pool = SqlitePool::connect(&db_url).await?;

    MIGRATOR.run(&pool).await?;

    let admin_exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE username = 'admin'")
            .fetch_optional(&pool)
            .await?;

    if admin_exists.is_none() {
        println!("Admin user not found, creating default admin user...");

        // Obtener credenciales del usuario inicial desde variables de entorno
        let initial_email = std::env::var("INITIAL_USER_EMAIL")
            .unwrap_or_else(|_| "admin@example.com".to_string());
        let initial_password = std::env::var("INITIAL_USER_PASSWORD")
            .unwrap_or_else(|_| "admin".to_string());

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(initial_password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        let admin_role_id = "7091390e-5cec-47d7-9d39-4f068d945788";

        sqlx::query(
            "INSERT INTO users (id, username, email, password, groups) VALUES (?, 'admin', ?, ?, ?)"
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&initial_email)
        .bind(password_hash)
        .bind(admin_role_id)
        .execute(&pool)
        .await?;

        println!("Created admin user with email: {}", initial_email);
    }

    println!("Database initialized successfully.");

    Ok(pool)
}
