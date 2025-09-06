use crate::{
    auth::Group,
    config::layers::{
        create_layer, delete_layer, get_layers, switch_layer_published, update_layer,
    },
    error::AppResult,
    models::category::Category,
};
use html_escape::encode_safe;
use serde::{Deserialize, Serialize};

pub enum StateLayer {
    Any,
    Published,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layer {
    pub id: String,
    pub category: Category,
    pub geometry: String,
    pub name: String,
    pub alias: String,
    pub description: String,
    pub schema: String,
    pub table_name: String,
    pub fields: Vec<String>,
    pub filter: Option<String>,
    pub srid: Option<u32>,
    pub geom: Option<String>,
    pub sql_mode: Option<String>,
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
    pub max_records: Option<u64>,
    pub published: bool,
    #[serde(rename = "source")]
    pub url: Option<String>,
    pub groups: Option<Vec<Group>>,
}

impl Layer {
    pub fn get_geom(&self) -> String {
        self.geom.as_deref().unwrap_or("geom").to_string()
    }

    pub fn get_sql_mode(&self) -> String {
        self.sql_mode.as_deref().unwrap_or("CTE").to_string()
    }

    pub fn get_filter(&self) -> String {
        self.filter.as_deref().unwrap_or("").to_string()
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

    pub fn get_max_records(&self) -> u64 {
        self.max_records.unwrap_or(0)
    }

    pub fn info_html(&self) -> String {
        let mut rv = format!("<strong>ID:</strong> {}<br>", self.id);
        rv += &format!("<strong>Name:</strong> {}<br>", self.name);
        rv += &format!("<strong>Alias:</strong> {}<br>", self.alias);
        rv += &format!(
            "<strong>Description:</strong> {}<br>",
            encode_safe(&self.description.clone())
        );
        rv += &format!("<strong>Schema:</strong> {}<br>", self.schema);
        rv += &format!("<strong>Table:</strong> {}<br>", self.table_name);
        rv += &format!(
            "<strong>Fields:</strong> {}<br>",
            encode_safe(&self.fields.join(", "))
        );
        rv += &format!("<strong>Field geom:</strong> {}<br>", self.get_geom());
        rv += &format!("<strong>SQL Mode:</strong> {}<br>", self.get_sql_mode());
        rv += &format!("<strong>SRID:</strong> {}<br>", self.get_srid());
        rv += &format!(
            "<strong>Filter:</strong> {}<br>",
            encode_safe(&self.get_filter())
        );
        rv += &format!("<strong>Buffer:</strong> {}<br>", self.get_buffer());
        rv += &format!("<strong>Extent:</strong> {}<br>", self.get_extent());
        rv += &format!("<strong>Zmin:</strong> {}<br>", self.get_zmin());
        rv += &format!("<strong>Zmax:</strong> {}<br>", self.get_zmax());
        rv += &format!(
            "<strong>Zmax do not simplify:</strong> {}<br>",
            self.get_zmax_do_not_simplify()
        );
        rv += &format!(
            "<strong>Buffer do not simplify:</strong> {}<br>",
            self.get_buffer_do_not_simplify()
        );
        rv += &format!(
            "<strong>Extent do not simplify:</strong> {}<br>",
            self.get_extent_do_not_simplify()
        );
        rv += &format!("<strong>Clip geom:</strong> {}<br>", self.get_clip_geom());
        rv += &format!(
            "<strong>Delete cache on start:</strong> {}<br>",
            self.get_delete_cache_on_start()
        );
        rv += &format!(
            "<strong>Max cache age:</strong> {}<br>",
            self.get_max_cache_age()
        );
        rv += &format!(
            "<strong>Max records:</strong> {}<br>",
            self.get_max_records()
        );
        rv += &format!("<strong>Published:</strong> {}<br>", self.published);
        rv += &format!(
            "<strong>Allowed Groups: </strong> {}",
            self.groups_as_string()
        );
        rv = rv.replace("\n", "\\n").replace("\r", "");
        rv
    }

    pub fn groups_as_string(&self) -> String {
        self.groups
            .as_ref()
            .map(|groups| {
                groups
                    .iter()
                    .map(|group| group.name.clone())
                    .collect::<Vec<String>>()
                    .join(" | ")
            })
            .unwrap_or_default()
    }

    pub fn groups_as_vec_string(&self) -> Vec<String> {
        self.groups
            .as_ref()
            .map(|groups| {
                groups
                    .iter()
                    .map(|group| group.name.clone())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default()
    }

    pub fn is_admin(&self) -> bool {
        self.groups_as_vec_string().contains(&"admin".to_string())
    }

    pub fn sort_by_category_and_name(layers: &mut Vec<Layer>) {
        layers.sort_by(|a, b| {
            a.category
                .name
                .to_lowercase()
                .cmp(&b.category.name.to_lowercase())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Catalog {
    pub layers: Vec<Layer>,
}

impl Catalog {
    pub async fn new(pool: &sqlx::SqlitePool) -> AppResult<Self> {
        let layers = get_layers(Some(pool)).await?;
        // layers.sort_by(|a, b| {
        //     a.category
        //         .name
        //         .to_lowercase()
        //         .cmp(&b.category.name.to_lowercase())
        //         .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        // });

        Ok(Self { layers })
    }

    pub fn find_layer_by_name<'a>(
        &'a self,
        target_name: &'a str,
        state: StateLayer,
    ) -> Option<&'a Layer> {
        match state {
            StateLayer::Any => self.layers.iter().find(|layer| layer.name == target_name),
            StateLayer::Published => self
                .layers
                .iter()
                .find(|layer| layer.name == target_name && layer.published),
        }
    }

    pub fn find_layer_by_category_and_name<'a>(
        &'a self,
        target_category: &'a str,
        target_name: &'a str,
        state: StateLayer,
    ) -> Option<&'a Layer> {
        match state {
            StateLayer::Any => self
                .layers
                .iter()
                .find(|layer| layer.category.name == target_category && layer.name == target_name),
            StateLayer::Published => self.layers.iter().find(|layer| {
                layer.category.name == target_category
                    && layer.name == target_name
                    && layer.published
            }),
        }
    }

    pub fn find_layers_by_category<'a>(
        &'a self,
        target_category: &'a str,
        state: StateLayer,
    ) -> Vec<&'a Layer> {
        self.layers
            .iter()
            .filter(|layer| match state {
                StateLayer::Any => layer.category.name == target_category,
                StateLayer::Published => layer.category.name == target_category && layer.published,
            })
            .collect()
    }

    pub fn find_layer_position_by_name(
        &self,
        target_name: &str,
        state: StateLayer,
    ) -> Option<usize> {
        match state {
            StateLayer::Any => self
                .layers
                .iter()
                .position(|layer| layer.name == target_name),
            StateLayer::Published => self
                .layers
                .iter()
                .position(|layer| layer.name == target_name && layer.published),
        }
    }

    pub fn find_layer_by_id<'a>(
        &'a self,
        target_id: &'a str,
        state: StateLayer,
    ) -> Option<&'a Layer> {
        match state {
            StateLayer::Any => self.layers.iter().find(|layer| layer.id == target_id),
            StateLayer::Published => self
                .layers
                .iter()
                .find(|layer| layer.id == target_id && layer.published),
        }
    }

    pub async fn swich_layer_published(&mut self, target_id: &str) -> AppResult<()> {
        switch_layer_published(None, target_id).await?;
        let position = self.layers.iter().position(|layer| layer.id == target_id);
        match position {
            Some(index) => self.layers[index].published = !self.layers.clone()[index].published,
            None => println!("layer not found"),
        }
        Ok(())
    }

    pub async fn add_layer(&mut self, layer: Layer) -> AppResult<()> {
        create_layer(None, layer.clone()).await?;
        self.layers.push(layer);
        Ok(())
    }

    pub async fn update_layer(&mut self, layer: Layer) -> AppResult<()> {
        update_layer(None, layer.clone()).await?;
        let position = self.layers.iter().position(|lyr| lyr.id == layer.id);
        match position {
            Some(index) => self.layers[index] = layer,
            None => println!("layer not found"),
        }

        Ok(())
    }

    pub async fn delete_layer(&mut self, id: String) -> AppResult<()> {
        delete_layer(None, id.as_str()).await?;
        self.layers.retain(|lyr| lyr.id != id);
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
