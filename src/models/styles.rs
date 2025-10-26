use crate::{
    config::styles::{
        create_style, delete_style, get_style, get_style_by_category_and_name, get_styles,
        update_style,
    },
    error::AppResult,
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
