use crate::error::AppResult;
use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub status: u16,
    pub message: String,
}

#[handler]
pub async fn handle_errors(res: &mut Response, ctrl: &mut FlowCtrl) -> AppResult<()> {
    if let Some(status) = res.status_code
        && status.as_u16() >= 400
        && status.as_u16() <= 600
    {
        let template = ErrorTemplate {
            status: status.as_u16(),
            message: status.canonical_reason().unwrap().to_string(),
        };

        res.render(Text::Html(template.render()?));
        ctrl.skip_rest();
        return Ok(());
    }

    Ok(())
}
