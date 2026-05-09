use super::utils::{BaseTemplateData, make_base};
use crate::error::{AppError, AppResult};
use crate::get_map_assets;
use askama::Template;
use salvo::prelude::*;
use std::collections::HashSet;
use tokio::fs;

#[derive(Template)]
#[template(path = "sprites/index.html")]
struct SpritesTemplate {
    base: BaseTemplateData,
    sprites: Vec<String>,
}

#[derive(Template)]
#[template(path = "glyphs/index.html")]
struct GlyphsTemplate {
    base: BaseTemplateData,
    glyphs: Vec<String>,
}

#[handler]
pub async fn page_sprites(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let dir = format!("{}/sprites", get_map_assets());
    let dir_path = dir.as_str();

    let mut entries = match fs::read_dir(dir_path).await {
        Ok(e) => e,
        Err(_) => {
            res.status_code(StatusCode::NOT_FOUND);
            return Err(AppError::NotFound(format!(
                "The directory {dir_path} does not exist"
            )));
        }
    };

    let mut unique_names: HashSet<String> = HashSet::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir()
            && let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
        {
            unique_names.insert(dir_name.to_string());
        }
    }

    let mut sprites: Vec<String> = unique_names.into_iter().collect();
    sprites.sort();
    let template = SpritesTemplate { base, sprites };

    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn page_glyphs(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let dir = format!("{}/glyphs", get_map_assets());
    let dir_path = dir.as_str();

    let mut entries = match fs::read_dir(dir_path).await {
        Ok(e) => e,
        Err(_) => {
            res.status_code(StatusCode::NOT_FOUND);
            return Err(AppError::NotFound(format!(
                "The directory {dir_path} does not exist"
            )));
        }
    };

    let mut unique_names: HashSet<String> = HashSet::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir()
            && let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
        {
            unique_names.insert(dir_name.to_string());
        }
    }
    let mut glyphs: Vec<String> = unique_names.into_iter().collect();
    glyphs.sort();
    let template = GlyphsTemplate { base, glyphs };
    res.render(Text::Html(template.render()?));
    Ok(())
}
