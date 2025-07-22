use crate::args::AppConfig;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
};
use inquire::{Confirm, CustomUserError, Select, Text, error::InquireError};
use std::io::stdout;

use super::config::{env_exists, save_config_to_env};

pub fn start_cli(mut appconfig: AppConfig) -> Result<(), CustomUserError> {
    let env_path = ".env";

    if !env_exists(env_path) {
        let create = Confirm::new("No .env file found. Do you want to create one?")
            .with_default(true)
            .prompt()?;

        if !create {
            println!("Configuration cancelled.");
            return Ok(());
        }
    }

    let options = vec![
        "Config directory",
        "Cache directory",
        "Map assets directory",
        "Server host",
        "Server port",
        "Database connection string",
        "Redis connection string",
        "JWT secret key",
        "Session secret key",
        "Database pool size (min)",
        "Database pool size (max)",
        "Save and exit",
        "Exit without saving",
    ];

    loop {
        clear_screen().expect("Failed to clear screen");
        let select = Select::new("Select field to edit:", options.clone()).prompt();

        match select {
            Ok(choice) => match choice {
                "Config directory" => {
                    appconfig.config_dir = prompt_edit("Config directory:", &appconfig.config_dir)?;
                }
                "Cache directory" => {
                    appconfig.cache_dir = prompt_edit("Cache directory:", &appconfig.cache_dir)?;
                }
                "Map assets directory" => {
                    appconfig.map_assets_dir =
                        prompt_edit("Map assets directory:", &appconfig.map_assets_dir)?;
                }
                "Server host" => {
                    appconfig.host = prompt_edit("Server host:", &appconfig.host)?;
                }
                "Server port" => {
                    appconfig.port = prompt_edit("Server port:", &appconfig.port)?;
                }
                "Database connection string" => {
                    appconfig.db_conn =
                        prompt_edit("Database connection string:", &appconfig.db_conn)?;
                }
                "Redis connection string" => {
                    appconfig.redis_conn =
                        prompt_edit("Redis connection string:", &appconfig.redis_conn)?;
                }
                "JWT secret key" => {
                    appconfig.jwt_secret = prompt_edit("JWT secret key:", &appconfig.jwt_secret)?;
                }
                "Session secret key" => {
                    appconfig.session_secret =
                        prompt_edit("Session secret key:", &appconfig.session_secret)?;
                }
                "Database pool size (min)" => {
                    let val = prompt_edit(
                        "Database pool size (min):",
                        &appconfig.db_pool_size_min.to_string(),
                    )?;
                    appconfig.db_pool_size_min = val.parse().unwrap_or(appconfig.db_pool_size_min);
                }
                "Database pool size (max)" => {
                    let val = prompt_edit(
                        "Database pool size (max):",
                        &appconfig.db_pool_size_max.to_string(),
                    )?;
                    appconfig.db_pool_size_max = val.parse().unwrap_or(appconfig.db_pool_size_max);
                }
                "Save and exit" => {
                    save_config_to_env(&appconfig, env_path).expect("Failed to save .env");
                    println!("Configuration saved. Exiting...");
                    break;
                }
                "Exit without saving" => {
                    println!("Exiting without saving...");
                    break;
                }
                _ => {}
            },
            Err(e) => match e {
                InquireError::OperationCanceled => {
                    println!("Operation cancelled by user.");
                    break;
                }
                InquireError::OperationInterrupted => {
                    println!("Operation interrupted. Exiting...");
                    break;
                }
                _ => {
                    eprintln!("An error occurred: {}", e);
                    continue;
                }
            },
        }
    }

    Ok(())
}

fn prompt_edit(prompt: &str, current_value: &str) -> Result<String, CustomUserError> {
    Ok(Text::new(prompt)
        .with_initial_value(current_value)
        .prompt()?)
}

fn clear_screen() -> std::io::Result<()> {
    let mut stdout = stdout();
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    Ok(())
}
