use serde::{Deserialize, Serialize};

use crate::{
    config::categories::{create_category, delete_category, get_category_by_id, update_category},
    error::AppResult,
    get_app_state,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl Category {
    pub async fn new(name: String, description: String) -> AppResult<Self> {
        let category = Category {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
        };

        create_category(None, category.clone()).await?;

        get_app_state().categories.push(category.clone());

        Ok(category)
    }

    pub async fn from_id(id: &str) -> AppResult<Self> {
        let category = get_category_by_id(None, id).await?;

        Ok(category)
    }

    pub async fn update_category(&self, name: String, description: String) -> AppResult<Self> {
        let category = Category {
            id: self.id.clone(),
            name,
            description,
        };

        update_category(None, category.clone()).await?;

        let position = get_app_state()
            .categories
            .iter()
            .position(|c| c.id == self.id);

        match position {
            Some(pos) => {
                get_app_state().categories[pos] = category.clone();
            }
            None => {
                get_app_state().categories.push(category.clone());
            }
        }

        let position = get_app_state()
            .catalog
            .layers
            .iter()
            .position(|l| l.category.id == self.id);

        match position {
            Some(pos) => {
                get_app_state().catalog.layers[pos].category = category.clone();
            }
            None => {}
        }

        Ok(category)
    }

    pub async fn delete_category(&self) -> AppResult<()> {
        delete_category(None, &self.id.clone()).await?;

        let position = get_app_state()
            .categories
            .iter()
            .position(|c| c.id == self.id);

        match position {
            Some(pos) => {
                get_app_state().categories.remove(pos);
            }
            None => {}
        }

        Ok(())
    }
}
