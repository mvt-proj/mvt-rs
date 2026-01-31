use askama::Template;
use salvo::prelude::*;

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub status: u16,
    pub message: String,
}

impl ErrorTemplate {
    pub fn render_or_fallback(&self) -> String {
        match self.render() {
            Ok(html) => html,
            Err(_) => format!("Error {}: {}", self.status, self.message),
        }
    }
}

#[handler]
pub async fn handle_errors(res: &mut Response, ctrl: &mut FlowCtrl) {
    if let Some(status) = res.status_code {
        if status.is_client_error() || status.is_server_error() {
            let template = ErrorTemplate {
                status: status.as_u16(),
                message: status.canonical_reason().unwrap_or("Error").to_string(),
            };

            if let Ok(html) = template.render() {
                res.render(Text::Html(html));
                ctrl.skip_rest();
            }
        }
    }
}
