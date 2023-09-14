use serde::Deserialize;
use std::fs;
use std::io;
use std::path::PathBuf;

fn read_json_from_file(file: PathBuf) -> Result<Layer, io::Error> {
    let json_string = fs::read_to_string(file)?;
    let layer: Layer = serde_json::from_str(&json_string)?;
    Ok(layer)
}

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Clone)]
pub struct LayersConfig {
    pub layers: Vec<Layer>,
}

impl LayersConfig {
    pub async fn new() -> Result<Self, io::Error> {
        let directory_layers = "layers";
        let mut layers: Vec<Layer> = Vec::new();

        let entries = fs::read_dir(directory_layers)?;

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    match read_json_from_file(path) {
                        Ok(layer) => layers.push(layer),
                        Err(_) => tracing::error!("Error reading the layer configuration"),
                    }
                }
            }
        }

        Ok(Self { layers })
    }

    pub fn find_layer_by_name<'a>(&'a self, target_name: &'a str) -> Option<&'a Layer> {
        self.layers.iter().find(|layer| layer.name == target_name)
    }
}
