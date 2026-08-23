use serde::{Deserialize, Serialize};

use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::{AppError, AppResult},
    get_catalog, get_categories, get_styles_cache,
    models::{catalog::Layer, styles::Style},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Counts how many layers and styles still reference `category_id`. Pure so
/// it can be unit-tested without the global `Catalog`/`STYLES` state that
/// `Category::delete_category` reads it from.
pub fn count_category_references(category_id: &str, layers: &[Layer], styles: &[Style]) -> (usize, usize) {
    let layer_count = layers.iter().filter(|l| l.category.id == category_id).count();
    let style_count = styles.iter().filter(|s| s.category.id == category_id).count();
    (layer_count, style_count)
}

impl Category {
    pub async fn new(name: String, description: String) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let category = Category {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
        };

        create_category(None, category.clone()).await?;
        let mut categories = get_categories().await.write().await;

        categories.push(category.clone());
        categories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(category)
    }

    pub async fn from_id(id: &str) -> AppResult<Self> {
        match get_category_by_id(None, id).await {
            Ok(category) => Ok(category),
            Err(sqlx::Error::RowNotFound) => {
                Err(AppError::NotFound(format!("Category {id} not found")))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn update_category(&self, name: String, description: String) -> AppResult<Self> {
        let name = crate::services::utils::normalize_name(&name)?;
        let category = Category {
            id: self.id.clone(),
            name,
            description,
        };

        update_category(None, category.clone()).await?;
        let mut categories = get_categories().await.write().await;

        let position = categories.iter().position(|c| c.id == self.id);

        match position {
            Some(pos) => {
                categories[pos] = category.clone();
            }
            None => {
                categories.push(category.clone());
            }
        }

        let mut catalog = get_catalog().await.write().await;

        for layer in catalog
            .layers
            .iter_mut()
            .filter(|l| l.category.id == self.id)
        {
            layer.category = category.clone();
        }

        categories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        crate::reload_styles_cache().await?;

        Ok(category)
    }

    pub async fn delete_category(&self) -> AppResult<()> {
        let catalog = get_catalog().await.read().await;
        let layers = catalog.layers.clone();
        drop(catalog);

        let styles_cache = get_styles_cache().await.read().await;
        let styles = styles_cache.clone();
        drop(styles_cache);

        let (layer_count, style_count) = count_category_references(&self.id, &layers, &styles);

        if layer_count > 0 || style_count > 0 {
            return Err(AppError::Conflict(format!(
                "Category '{}' is in use by {layer_count} layer(s) and {style_count} style(s)",
                self.name
            )));
        }

        delete_category(None, &self.id.clone()).await?;
        let mut categories = get_categories().await.write().await;

        let position = categories.iter().position(|c| c.id == self.id);

        if let Some(pos) = position {
            categories.remove(pos);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_support::in_memory_pool;
    use crate::models::catalog::Layer;
    use crate::models::styles::Style;

    fn test_layer_with_category(id: &str, category_id: &str) -> Layer {
        Layer {
            id: id.to_string(),
            category: Category {
                id: category_id.to_string(),
                name: format!("cat-{category_id}"),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "layer".to_string(),
            alias: "Layer".to_string(),
            description: String::new(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "t".to_string(),
            fields: vec![],
            filter: None,
            srid: None,
            geom: None,
            label_layer: false,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age: None,
            max_records: None,
            published: true,
            url: None,
            groups: None,
        }
    }

    fn test_style_with_category(id: &str, category_id: &str) -> Style {
        Style {
            id: id.to_string(),
            name: "style".to_string(),
            category: Category {
                id: category_id.to_string(),
                name: format!("cat-{category_id}"),
                description: String::new(),
            },
            description: String::new(),
            style: "{}".to_string(),
        }
    }

    #[test]
    fn test_count_category_references_none() {
        let layers = vec![test_layer_with_category("l1", "other")];
        let styles = vec![test_style_with_category("s1", "other")];
        assert_eq!(count_category_references("target", &layers, &styles), (0, 0));
    }

    #[test]
    fn test_count_category_references_counts_layers_and_styles() {
        let layers = vec![
            test_layer_with_category("l1", "target"),
            test_layer_with_category("l2", "target"),
            test_layer_with_category("l3", "other"),
        ];
        let styles = vec![test_style_with_category("s1", "target")];
        assert_eq!(count_category_references("target", &layers, &styles), (2, 1));
    }

    #[test]
    fn test_count_category_references_empty_input() {
        assert_eq!(count_category_references("target", &[], &[]), (0, 0));
    }

    #[tokio::test]
    async fn get_category_by_id_returns_row_not_found_for_unknown_id() {
        let pool = in_memory_pool().await;
        let result = crate::config::categories::get_category_by_id(Some(&pool), "missing-id").await;
        assert!(matches!(result, Err(sqlx::Error::RowNotFound)));
    }
}
