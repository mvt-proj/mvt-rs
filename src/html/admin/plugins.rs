use askama::Template;
use salvo::prelude::*;

use crate::{
    config,
    error::{AppError, AppResult},
    get_cf_pool, get_plugin_registry,
    html::utils::{BaseTemplateData, make_base},
    plugins::PluginInfo,
};

#[derive(Template)]
#[template(path = "admin/plugins.html")]
struct PluginsTemplate {
    base: BaseTemplateData,
    plugins: Vec<PluginInfo>,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let registry = get_plugin_registry();
    let plugins = registry.list_plugins_with_status();

    let template = PluginsTemplate { base, plugins };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn toggle(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let key = req
        .param::<String>("key")
        .ok_or(AppError::RequestParamError("key".to_string()))?;

    let registry = get_plugin_registry();
    let new_state = !registry.is_plugin_enabled(&key);

    config::plugins::set_plugin_enabled(Some(get_cf_pool()), &key, new_state).await?;
    registry.set_plugin_enabled(&key, new_state);

    res.render(Redirect::other("/admin/plugins"));
    Ok(())
}
