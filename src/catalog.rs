use serde::{Deserialize, Serialize};

// use std::path::Path;
// use tokio::fs::File;
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::storage::Storage;

pub enum StateLayer {
    ANY,
    PUBLISHED,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layer {
    pub geometry: String,
    pub name: String,
    pub alias: String,
    pub schema: String,
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

impl Layer {
    pub fn get_geom(&self) -> String {
        self.geom
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("geom")
            .to_string()
    }

    pub fn get_filter(&self) -> String {
        self.filter
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn get_srid(&self) -> u32 {
        self.srid.unwrap_or(4326)
    }

    pub fn get_buffer(&self) -> u32 {
        self.buffer.unwrap_or(256)
    }

    pub fn get_extent(&self) -> u32 {
        self.extent.unwrap_or(4096)
    }

    pub fn get_zmin(&self) -> u32 {
        self.zmin.unwrap_or(0)
    }

    pub fn get_zmax(&self) -> u32 {
        self.zmax.unwrap_or(22)
    }

    pub fn get_zmax_do_not_simplify(&self) -> u32 {
        self.zmax_do_not_simplify.unwrap_or(16)
    }

    pub fn get_buffer_do_not_simplify(&self) -> u32 {
        self.buffer_do_not_simplify.unwrap_or(256)
    }

    pub fn get_extent_do_not_simplify(&self) -> u32 {
        self.extent_do_not_simplify.unwrap_or(4096)
    }

    pub fn get_clip_geom(&self) -> bool {
        self.clip_geom.unwrap_or(true)
    }

    pub fn get_delete_cache_on_start(&self) -> bool {
        self.delete_cache_on_start.unwrap_or(false)
    }

    pub fn get_max_cache_age(&self) -> u64 {
        self.max_cache_age.unwrap_or(0)
    }

    pub fn info_html(&self) -> String {
        let mut rv = format!("Name: {}<br>", self.name);
        rv += &format!("Alias: {}<br>", self.alias);
        rv += &format!("Schema: {}<br>", self.schema);
        rv += &format!("Table: {}<br>", self.table);
        rv += &format!("Fields: {}<br>", self.fields.join(", "));
        rv += &format!("Field geom: {}<br>", self.get_geom());
        rv += &format!("SRID: {}<br>", self.get_srid());
        rv += &format!("Filter: {}<br>", self.get_filter());
        rv += &format!("Buffer: {}<br>", self.get_buffer());
        rv += &format!("Extent: {}<br>", self.get_extent());
        rv += &format!("Zmin: {}<br>", self.get_zmin());
        rv += &format!("Zmax: {}<br>", self.get_zmax());
        rv += &format!(
            "Zmax do not simplify: {}<br>",
            self.get_zmax_do_not_simplify()
        );
        rv += &format!(
            "Buffer do not simplify: {}<br>",
            self.get_buffer_do_not_simplify()
        );
        rv += &format!(
            "Extent do not simplify: {}<br>",
            self.get_extent_do_not_simplify()
        );
        rv += &format!("Clip geom: {}<br>", self.get_clip_geom());
        rv += &format!(
            "Delete cache on start: {}<br>",
            self.get_delete_cache_on_start()
        );
        rv += &format!("Max cache age: {}<br>", self.get_max_cache_age());
        rv += &format!("Published: {}", self.published);
        rv
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Catalog {
    pub layers: Vec<Layer>,
    pub config_dir: String,
    pub storage_path: String,
}

impl Catalog {
    pub async fn new(config_dir: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let storage_path = format!("{config_dir}/catalog.json");
        let mut storage = Storage::<Vec<Layer>>::new(storage_path.clone());
        let loaded_catalog = storage.load().await?;
        let layers: Vec<Layer> = loaded_catalog.unwrap_or(Vec::new());

        Ok(Self {
            layers,
            config_dir: config_dir.to_string(),
            storage_path,
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
                .layers
                .iter()
                .find(|layer| layer.name == target_name && layer.published),
        }
    }

    pub fn find_layer_position_by_name(
        &self,
        target_name: &str,
        state: StateLayer,
    ) -> Option<usize> {
        match state {
            StateLayer::ANY => self
                .layers
                .iter()
                .position(|layer| layer.name == target_name),
            StateLayer::PUBLISHED => self
                .layers
                .iter()
                .position(|layer| layer.name == target_name && layer.published),
        }
    }

    pub async fn swich_layer_published(&mut self, target_name: &str) {
        let position = self
            .layers
            .iter()
            .position(|layer| layer.name == target_name);
        match position {
            Some(index) => self.layers[index].published = !self.layers.clone()[index].published,
            None => println!("layer not found"),
        }
        let mut storage = Storage::<Vec<Layer>>::new(self.storage_path.clone());
        storage.save(self.layers.clone()).await.unwrap();
    }

    pub async fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
        let mut storage = Storage::<Vec<Layer>>::new(self.storage_path.clone());
        storage.save(self.layers.clone()).await.unwrap();
    }

    pub async fn update_layer(&mut self, layer: Layer) {
        let position = self.layers.iter().position(|lyr| lyr.name == layer.name);
        match position {
            Some(index) => self.layers[index] = layer,
            None => println!("layer not found"),
        }
        let mut storage = Storage::<Vec<Layer>>::new(self.storage_path.clone());
        storage.save(self.layers.clone()).await.unwrap();
    }

    pub async fn delete_layer(
        &mut self,
        name: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.layers.retain(|lyr| lyr.name != name);
        let mut storage = Storage::<Vec<Layer>>::new(self.storage_path.clone());
        storage.save(self.layers.clone()).await?;
        Ok(())
    }

    pub fn get_published_layers(&self) -> Vec<Layer> {
        self.layers
            .iter()
            .filter(|layer| layer.published)
            .cloned()
            .collect()
    }

    pub fn remove_layer_by_name(&mut self, target_name: &str) {
        self.layers.retain(|layer| layer.name != target_name);
    }
}
