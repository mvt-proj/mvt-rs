use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use sqlx::{Connection, Executor, SqliteConnection, SqlitePool};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub async fn init_sqlite(db_path: &str, salt: String) -> Result<SqlitePool, sqlx::Error> {
    if !Path::new(db_path).exists() {
        println!("Database file not found, initializing: {}", db_path);

        if let Some(parent) = Path::new(db_path).parent() {
            fs::create_dir_all(parent).expect("Failed to create database directory");
            fs::File::create(db_path).expect("Failed to create database file");
        }

        let mut conn = SqliteConnection::connect(&format!("sqlite:{}", db_path)).await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                        id TEXT PRIMARY KEY NOT NULL,
                        username TEXT NOT NULL,
                        email TEXT NOT NULL UNIQUE,
                        password TEXT NOT NULL,
                        groups TEXT NOT NULL
                    );",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS categories (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL UNIQUE,
                        description TEXT NOT NULL
                    );",
        )
        .await?;

        let public_category_id = Uuid::new_v4().to_string();

        conn.execute(
            format!(
                "
            INSERT INTO categories (id, name, description)
            VALUES ('{}', 'public', 'public category');
        ",
                public_category_id
            )
            .as_str(),
        )
        .await?;

        conn.execute(
            "
            CREATE TABLE layers (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                geometry TEXT NOT NULL,
                name TEXT NOT NULL,
                alias TEXT NOT NULL,
                schema TEXT NOT NULL,
                table_name TEXT NOT NULL,
                fields TEXT NOT NULL,
                filter TEXT,
                srid INTEGER,
                geom TEXT,
                sql_mode TEXT,
                buffer INTEGER,
                extent INTEGER,
                zmin INTEGER,
                zmax INTEGER,
                zmax_do_not_simplify INTEGER,
                buffer_do_not_simplify INTEGER,
                extent_do_not_simplify INTEGER,
                clip_geom BOOLEAN,
                delete_cache_on_start BOOLEAN,
                max_cache_age INTEGER,
                published BOOLEAN NOT NULL,
                url TEXT,
                groups TEXT NOT NULL
            );
        ",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS groups (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL UNIQUE,
                        description TEXT NOT NULL
                    );",
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS styles (
                        id TEXT PRIMARY KEY NOT NULL,
                        category TEXT NOT NULL,
                        name TEXT NOT NULL,
                        description TEXT NOT NULL,
                        style TEXT NOT NULL
                    );",
        )
        .await?;

        let admin_role_id = Uuid::new_v4().to_string();

        conn.execute(
            format!(
                "
            INSERT INTO groups (id, name, description)
            VALUES ('{}', 'admin', 'admin role');
        ",
                admin_role_id
            )
            .as_str(),
        )
        .await?;

        conn.execute(
            format!(
                "
            INSERT INTO groups (id, name, description)
            VALUES ('{}', 'operator', 'operator role');
        ",
                Uuid::new_v4()
            )
            .as_str(),
        )
        .await?;

        //create admin user with conn.execute
        let argon2 = Argon2::default();
        let salt = SaltString::encode_b64(salt.as_bytes()).unwrap();
        let password_hash = argon2
            .hash_password("admin".to_string().as_bytes(), &salt)
            .unwrap()
            .to_string();
        conn.execute(
            format!(
                "
            INSERT INTO users
                (id, username, email, password, groups)
            VALUES
                ('{}', 'admin', 'admin@mail.com', '{password_hash}', '{admin_role_id}');",
                Uuid::new_v4(),
            )
            .as_str(),
        )
        .await?;

        println!("Database initialized successfully.");
    } else {
        println!("Database file found, skipping initialization.");
    }

    // Crea el pool de conexiones
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path)).await?;
    Ok(pool)
}
