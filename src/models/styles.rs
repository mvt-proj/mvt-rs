use crate::{
    config::styles::{
        create_style, delete_style, get_style, get_style_by_category_and_name, get_styles,
        update_style,
    },
    error::{AppError, AppResult},
    models::category::Category,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub description: String,
    pub style: String,
}

impl Style {
    pub async fn new(
        name: String,
        category: Category,
        description: String,
        style: String,
    ) -> AppResult<Self> {
        let style = Style {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            category,
            description,
            style,
        };

        create_style(style.clone(), None).await?;

        Ok(style)
    }

    pub fn to_json(&self) -> Value {
        serde_json::from_str(&self.style).unwrap_or_else(|_| json!({}))
    }

    pub fn is_map(&self) -> bool {
        let json_value = self.to_json();
        json_value.get("version").is_some()
    }

    pub async fn get_all_styles() -> AppResult<Vec<Self>> {
        let styles = get_styles(None).await?;

        Ok(styles)
    }

    pub async fn from_id(id: &str) -> AppResult<Self> {
        let style = get_style(id, None).await?;

        Ok(style)
    }

    pub async fn from_category_and_name(category: &str, name: &str) -> AppResult<Self> {
        let style = get_style_by_category_and_name(category, name, None).await?;

        Ok(style)
    }

    /// Reads from the in-memory styles cache (used by the public endpoints so
    /// instances without a SQLite — clients — can serve styles/legends).
    pub async fn from_category_and_name_cached(category: &str, name: &str) -> AppResult<Self> {
        let styles = crate::get_styles_cache().await.read().await;
        find_style(&styles, category, name)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("style {category}:{name}")))
    }

    pub async fn update_style(
        &self,
        name: String,
        category: Category,
        description: String,
        style: String,
    ) -> AppResult<Self> {
        let style = Style {
            id: self.id.clone(),
            name,
            category,
            description,
            style,
        };

        update_style(style.clone(), None).await?;

        Ok(style)
    }

    pub async fn delete_style(&self) -> AppResult<()> {
        delete_style(&self.id, None).await?;
        Ok(())
    }

    pub fn sort_by_category_and_name(styles: &mut [Style]) {
        styles.sort_by(|a, b| {
            a.category
                .name
                .to_lowercase()
                .cmp(&b.category.name.to_lowercase())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
}

/// Finds a style in a slice by its category name and style name. Pure so it can
/// be unit-tested without the global cache.
pub fn find_style<'a>(styles: &'a [Style], category: &str, name: &str) -> Option<&'a Style> {
    styles.iter().find(|s| s.category.name == category && s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::category::Category;

    fn style(cat: &str, name: &str) -> Style {
        Style {
            id: format!("{cat}-{name}"),
            name: name.into(),
            category: Category { id: cat.into(), name: cat.into(), description: String::new() },
            description: String::new(),
            style: "{}".into(),
        }
    }

    #[test]
    fn find_style_matches_category_name_and_style_name() {
        let styles = vec![style("roads", "default"), style("water", "blue")];
        assert_eq!(find_style(&styles, "water", "blue").unwrap().id, "water-blue");
        assert!(find_style(&styles, "water", "default").is_none());
        assert!(find_style(&styles, "nope", "blue").is_none());
    }
}
