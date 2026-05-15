use askama::Template;
use salvo::prelude::*;

use crate::{
    error::AppResult,
    get_plugin_registry,
    html::utils::{BaseTemplateData, make_base},
    plugins::PluginInfo,
};

#[derive(Template)]
#[template(path = "admin/plugins.html")]
struct PluginsTemplate<'a> {
    base: BaseTemplateData,
    plugins: &'a [PluginInfo],
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let registry = get_plugin_registry();
    let plugins = registry.list_plugins();

    let template = PluginsTemplate { base, plugins };
    res.render(Text::Html(template.render()?));
    Ok(())
}
