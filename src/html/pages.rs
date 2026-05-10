use super::utils::{BaseTemplateData, make_base};
use crate::error::AppResult;
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "changepassword.html")]
struct ChangePasswordTemplate {
    base: BaseTemplateData,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let template = IndexTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn login(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    if base.is_auth {
        res.render(Redirect::other("/"));
        return Ok(());
    }

    let template = LoginTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn change_password(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    if !base.is_auth {
        res.render(Redirect::other("/login"));
        return Ok(());
    }

    let template = ChangePasswordTemplate { base };
    res.render(Text::Html(template.render()?));
    Ok(())
}
