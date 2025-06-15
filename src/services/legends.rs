use crate::{error::AppResult, models::styles::Style};
use maplibre_legend::MapLibreLegend;
use salvo::prelude::*;

#[handler]
pub async fn index(req: &mut Request, res: &mut Response) -> AppResult<()> {
    res.headers_mut()
        .insert("content-type", "image/svg+xml".parse()?);

    let style_name = req.param::<String>("style_name").unwrap_or_default();
    let layer_id = req.query::<String>("layer_id").unwrap_or_default();
    let default_width = req.query::<u32>("width").unwrap_or(250);
    let default_height = req.query::<u32>("height").unwrap_or(40);
    let has_label = req.query::<bool>("has_label").unwrap_or_default();
    let include_raster = req.query::<bool>("include_raster").unwrap_or_default();
    let reverse = req.query::<bool>("reverse").unwrap_or_default();
    let parts: Vec<&str> = style_name.split(':').collect();

    let category = parts.first().unwrap_or(&"");
    let name = parts.get(1).unwrap_or(&"");
    let style = Style::from_category_and_name(category, name).await?;

    let legend = MapLibreLegend::new(
        &style.style,
        default_width,
        default_height,
        has_label,
        include_raster,
    )
    .await?;

    if !layer_id.is_empty() {
        let svg = legend.render_layer(&layer_id, Some(has_label))?;
        res.render(svg);
    } else {
        let svg = legend.render_all(reverse)?;
        res.render(svg);
    }
    Ok(())
}
