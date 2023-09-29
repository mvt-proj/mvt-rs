use serde::{Deserialize, Serialize};

use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub enum StateLayer {
    ANY,
    PUBLISHED,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layer {
    pub geometry: String,
    pub name: String,
    pub alias: String,
    pub schema: Option<String>,
    pub table: String,
    pub fields: Vec<String>,
    pub filter: Option<String>,
    pub srid: Option<u32>,
    pub geom: Option<String>,
    pub buffer: Option<u32>,
    pub extent: Option<u32>,
    pub zmin: Option<u32>,
    pub zmax: Option<u32>,
    /// zmax_do_not_simplify: maximum z value from which the buffer and extent will not use and will use the value of buffer_do_not_simplify and extent_do_not_simplify
    pub zmax_do_not_simplify: Option<u32>,
    pub buffer_do_not_simplify: Option<u32>,
    pub extent_do_not_simplify: Option<u32>,
    pub clip_geom: Option<bool>,
    pub delete_cache_on_start: Option<bool>,
    /// max_cache_age: on seconds: default 0 -> infinite
    pub max_cache_age: Option<u64>,
    pub published: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Catalog {
    pub layers: Vec<Layer>,
    pub published_layers: Vec<Layer>,
    pub config_dir: String,
}

impl Catalog {
    pub async fn new(config_dir: &str) -> Result<Self, anyhow::Error> {
        let file_path = Path::new(config_dir).join("catalog.json".to_string());
        let mut file = File::open(file_path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;

        let layers: Vec<Layer> = serde_json::from_str(&contents.clone())?;

        let published_layers: Vec<Layer> = layers
            .iter()
            .filter(|layer| layer.published)
            .cloned()
            .collect();

        Ok(Self {
            layers,
            published_layers,
            config_dir: config_dir.to_string(),
        })
    }

    pub fn find_layer_by_name<'a>(
        &'a self,
        target_name: &'a str,
        state: StateLayer,
    ) -> Option<&'a Layer> {
        match state {
            StateLayer::ANY => self.layers.iter().find(|layer| layer.name == target_name),
            StateLayer::PUBLISHED => self
                .published_layers
                .iter()
                .find(|layer| layer.name == target_name),
        }
    }
}
