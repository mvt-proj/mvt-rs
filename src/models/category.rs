use serde::{Deserialize, Serialize};

use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::{AppError, AppResult},
    get_catalog, get_categories,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
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

        Ok(category)
    }

    pub async fn delete_category(&self) -> AppResult<()> {
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
    use crate::config::test_support::in_memory_pool;

    #[tokio::test]
    async fn get_category_by_id_returns_row_not_found_for_unknown_id() {
        let pool = in_memory_pool().await;
        let result = crate::config::categories::get_category_by_id(Some(&pool), "missing-id").await;
        assert!(matches!(result, Err(sqlx::Error::RowNotFound)));
    }
}
