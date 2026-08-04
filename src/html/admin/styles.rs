use askama::Template;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::User,
    error::{AppError, AppResult},
    get_categories, get_catalog,
    html::utils::{BaseTemplateData, make_base},
    models::{category::Category, catalog::Layer, styles::Style},
};

#[derive(Template)]
#[template(path = "admin/styles/styles.html")]
struct ListStylesTemplate<'a> {
    current_user: &'a User,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/styles/new.html")]
struct NewStyleTemplate {
    categories: Vec<Category>,
    layers: Vec<Layer>,
    base: BaseTemplateData,
}

#[derive(Template)]
#[template(path = "admin/styles/edit.html")]
struct EditStyleTemplate {
    style: Style,
    style_json_safe: String,
    categories: Vec<Category>,
    layers: Vec<Layer>,
    base: BaseTemplateData,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct NewStyle<'a> {
    id: Option<String>,
    name: &'a str,
    category: String,
    description: &'a str,
    style: String,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct ConvertQmlRequest {
    qml: String,
    source_layer: String,
    mode: String,
}

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct ConvertSldRequest {
    sld: String,
    source_layer: String,
    mode: String,
}

#[handler]
pub async fn list_styles(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;
    let Some(current_user) = user else {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };
    let template = ListStylesTemplate {
        current_user: &current_user,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_style_page(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;

    let categories = get_categories().await.read().await;
    let mut layers = get_catalog().await.read().await.layers.clone();
    Layer::sort_by_category_and_name(&mut layers);
    let template = NewStyleTemplate {
        categories: (categories).to_vec(),
        layers,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_style_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = Style::from_id(&id).await?;
    let categories = get_categories().await.read().await;
    let mut layers = get_catalog().await.read().await.layers.clone();
    Layer::sort_by_category_and_name(&mut layers);
    // Escape "</" so the JSON can't break out of the <script type="application/json">
    // block it's embedded in (e.g. a style value containing "</script>"); JSON.parse
    // unescapes "\/" back to "/" so the parsed value is unaffected.
    let style_json_safe = style.style.replace("</", "<\\/");
    let template = EditStyleTemplate {
        style: style.clone(),
        style_json_safe,
        categories: (categories).to_vec(),
        layers,
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_style<'a>(res: &mut Response, style_form: NewStyle<'a>) -> AppResult<()> {
    if let Err(err) = crate::services::utils::validate_style_json(&style_form.style) {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    let category = match Category::from_id(&style_form.category).await {
        Ok(cat) => cat,
        Err(err) => {
            res.status_code(StatusCode::BAD_REQUEST);
            return Err(err);
        }
    };

    let result = Style::new(
        style_form.name.to_string(),
        category,
        style_form.description.to_string(),
        style_form.style.to_string(),
    )
    .await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn update_style<'a>(res: &mut Response, style_form: NewStyle<'a>) -> AppResult<()> {
    let style_id = style_form
        .id
        .ok_or(AppError::RequestParamError("id".to_string()))?;
    let style = match Style::from_id(&style_id).await {
        Ok(s) => s,
        Err(err) => {
            res.status_code(StatusCode::NOT_FOUND);
            return Err(err);
        }
    };

    let category = match Category::from_id(&style_form.category).await {
        Ok(cat) => cat,
        Err(err) => {
            res.status_code(StatusCode::BAD_REQUEST);
            return Err(err);
        }
    };

    if let Err(err) = crate::services::utils::validate_style_json(&style_form.style) {
        res.status_code(StatusCode::BAD_REQUEST);
        return Err(err);
    }

    let result = style
        .update_style(
            style_form.name.to_string(),
            category,
            style_form.description.to_string(),
            style_form.style.to_string(),
        )
        .await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

#[handler]
pub async fn delete_style(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let id = req
        .param::<String>("id")
        .ok_or(AppError::RequestParamError("id".to_string()))?;

    let style = match Style::from_id(&id).await {
        Ok(s) => s,
        Err(err) => {
            res.status_code(StatusCode::NOT_FOUND);
            return Err(err);
        }
    };

    let result = style.delete_style().await;

    if let Err(err) = result {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return Err(err);
    }

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/styles"));
    Ok(())
}

fn parse_output_mode(mode: &str) -> AppResult<qml2maplibre::OutputMode> {
    match mode {
        "compact" => Ok(qml2maplibre::OutputMode::Compact),
        "qgis" => Ok(qml2maplibre::OutputMode::QgisCompatible),
        other => Err(AppError::InvalidInput(format!(
            "unknown conversion mode '{other}'"
        ))),
    }
}

fn parse_sld_output_mode(mode: &str) -> AppResult<sld2maplibre::OutputMode> {
    match mode {
        "compact" => Ok(sld2maplibre::OutputMode::Compact),
        "qgis" => Ok(sld2maplibre::OutputMode::QgisCompatible),
        other => Err(AppError::InvalidInput(format!(
            "unknown conversion mode '{other}'"
        ))),
    }
}

#[handler]
pub async fn convert_qml(res: &mut Response, data: ConvertQmlRequest) -> AppResult<()> {
    if data.source_layer.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "source_layer must not be empty".to_string(),
        ));
    }

    let mode = parse_output_mode(&data.mode)?;
    let result = qml2maplibre::convert(&data.qml, &data.source_layer, mode)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    res.render(Json(serde_json::json!({
        "layers": result.layers,
        "warnings": result.warnings,
    })));
    Ok(())
}

#[handler]
pub async fn convert_sld(res: &mut Response, data: ConvertSldRequest) -> AppResult<()> {
    if data.source_layer.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "source_layer must not be empty".to_string(),
        ));
    }

    let mode = parse_sld_output_mode(&data.mode)?;
    let result = sld2maplibre::convert(&data.sld, &data.source_layer, mode)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    res.render(Json(serde_json::json!({
        "layers": result.layers,
        "warnings": result.warnings,
    })));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_mode_accepts_compact() {
        assert_eq!(
            parse_output_mode("compact").unwrap(),
            qml2maplibre::OutputMode::Compact
        );
    }

    #[test]
    fn parse_output_mode_accepts_qgis() {
        assert_eq!(
            parse_output_mode("qgis").unwrap(),
            qml2maplibre::OutputMode::QgisCompatible
        );
    }

    #[test]
    fn parse_output_mode_rejects_unknown_value() {
        assert!(parse_output_mode("bogus").is_err());
    }

    const SINGLESYMBOL_LINE_QML: &str = r#"<!DOCTYPE qgis PUBLIC 'http://mrcc.com/qgis.dtd' 'SYSTEM'>
<qgis>
  <renderer-v2 enableorderby="0" type="singleSymbol" symbollevels="0" forceraster="0">
    <symbols>
      <symbol type="line" alpha="1" force_rhr="0" name="0" clip_to_extent="1">
        <layer locked="0" pass="0" enabled="1" class="SimpleLine">
          <prop v="square" k="capstyle"/>
          <prop v="bevel" k="joinstyle"/>
          <prop v="9,113,185,255" k="line_color"/>
          <prop v="solid" k="line_style"/>
          <prop v="2" k="line_width"/>
          <prop v="Pixel" k="line_width_unit"/>
          <data_defined_properties>
            <Option type="Map">
              <Option type="QString" value="" name="name"/>
            </Option>
          </data_defined_properties>
        </layer>
      </symbol>
    </symbols>
  </renderer-v2>
</qgis>
"#;

    #[tokio::test]
    async fn convert_qml_returns_layers_for_valid_qml() {
        use salvo::test::{ResponseExt, TestClient};

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": SINGLESYMBOL_LINE_QML,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let mut resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
        let json: serde_json::Value = resp.take_json().await.unwrap();
        let layers = json["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0]["type"], "line");
        assert_eq!(layers[0]["source-layer"], "test_layer");
    }

    #[tokio::test]
    async fn convert_qml_rejects_malformed_xml() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": "<not valid xml",
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_qml_rejects_unknown_mode() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": SINGLESYMBOL_LINE_QML,
            "source_layer": "test_layer",
            "mode": "bogus",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_qml_rejects_empty_source_layer() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": SINGLESYMBOL_LINE_QML,
            "source_layer": "",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_qml_rejects_whitespace_only_source_layer() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": SINGLESYMBOL_LINE_QML,
            "source_layer": "   ",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    // Regression coverage for the route-level `SecureMaxSize` hoop added in
    // src/routes.rs: without it, a request body over Salvo's global 64 KB
    // default is rejected before the handler ever runs (real QML files with
    // categorized/graduated renderers routinely exceed that). With the hoop
    // in place, the same oversized-relative-to-64KB body is accepted.
    #[tokio::test]
    async fn convert_qml_with_secure_max_size_hoop_accepts_body_over_default_limit() {
        use salvo::http::request::SecureMaxSize;
        use salvo::test::TestClient;

        // Padding pushes the JSON body well past Salvo's 64 KB default.
        let big_qml = format!("{SINGLESYMBOL_LINE_QML}{}", " ".repeat(200_000));
        let service = Service::new(
            Router::with_path("convert-qml")
                .hoop(SecureMaxSize::new(8 * 1024 * 1024))
                .post(convert_qml),
        );
        let body = serde_json::json!({
            "qml": big_qml,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    }

    // Without the route-level hoop, Salvo silently discards the
    // oversized-body error internally (see `Request::payload()` /
    // `from_request()` in salvo_core): the request doesn't come back as a
    // 413, it surfaces as a 400 because the handler's required fields are
    // never populated. It also isn't rendered as JSON (mvt-rs's default
    // error catcher renders an HTML page for it since it never reaches our
    // `AppError` JSON `Writer`) — that combination (error status + non-JSON
    // body) is exactly what static/js/qml-import.js's error handling keys
    // off of for its "file too large" hint.
    #[tokio::test]
    async fn convert_qml_without_secure_max_size_hoop_rejects_body_over_default_limit() {
        use salvo::test::TestClient;

        let big_qml = format!("{SINGLESYMBOL_LINE_QML}{}", " ".repeat(200_000));
        let service = Service::new(Router::with_path("convert-qml").post(convert_qml));
        let body = serde_json::json!({
            "qml": big_qml,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-qml")
            .json(&body)
            .send(&service)
            .await;

        assert!(resp.status_code.unwrap().is_client_error());
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(
            !content_type.contains("application/json"),
            "expected a non-JSON error body, got content-type: {content_type}"
        );
    }

    #[test]
    fn parse_sld_output_mode_accepts_compact() {
        assert_eq!(
            parse_sld_output_mode("compact").unwrap(),
            sld2maplibre::OutputMode::Compact
        );
    }

    #[test]
    fn parse_sld_output_mode_accepts_qgis() {
        assert_eq!(
            parse_sld_output_mode("qgis").unwrap(),
            sld2maplibre::OutputMode::QgisCompatible
        );
    }

    #[test]
    fn parse_sld_output_mode_rejects_unknown_value() {
        assert!(parse_sld_output_mode("bogus").is_err());
    }

    const SINGLESYMBOL_LINE_SLD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor xmlns="http://www.opengis.net/sld" xmlns:se="http://www.opengis.net/se" xmlns:ogc="http://www.opengis.net/ogc" version="1.1.0">
  <NamedLayer>
    <se:Name>test_layer</se:Name>
    <UserStyle>
      <se:Name>test_layer</se:Name>
      <se:FeatureTypeStyle>
        <se:Rule>
          <se:Name>default</se:Name>
          <se:LineSymbolizer>
            <se:Stroke>
              <se:SvgParameter name="stroke">#0971b9</se:SvgParameter>
              <se:SvgParameter name="stroke-width">2</se:SvgParameter>
            </se:Stroke>
          </se:LineSymbolizer>
        </se:Rule>
      </se:FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>
"#;

    #[tokio::test]
    async fn convert_sld_returns_layers_for_valid_sld() {
        use salvo::test::{ResponseExt, TestClient};

        let service = Service::new(Router::with_path("convert-sld").post(convert_sld));
        let body = serde_json::json!({
            "sld": SINGLESYMBOL_LINE_SLD,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let mut resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
        let json: serde_json::Value = resp.take_json().await.unwrap();
        let layers = json["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0]["type"], "line");
        assert_eq!(layers[0]["source-layer"], "test_layer");
    }

    #[tokio::test]
    async fn convert_sld_rejects_malformed_xml() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-sld").post(convert_sld));
        let body = serde_json::json!({
            "sld": "<not valid xml",
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_sld_rejects_unknown_mode() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-sld").post(convert_sld));
        let body = serde_json::json!({
            "sld": SINGLESYMBOL_LINE_SLD,
            "source_layer": "test_layer",
            "mode": "bogus",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_sld_rejects_empty_source_layer() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-sld").post(convert_sld));
        let body = serde_json::json!({
            "sld": SINGLESYMBOL_LINE_SLD,
            "source_layer": "",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_sld_rejects_whitespace_only_source_layer() {
        use salvo::test::TestClient;

        let service = Service::new(Router::with_path("convert-sld").post(convert_sld));
        let body = serde_json::json!({
            "sld": SINGLESYMBOL_LINE_SLD,
            "source_layer": "   ",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn convert_sld_with_secure_max_size_hoop_accepts_body_over_default_limit() {
        use salvo::http::request::SecureMaxSize;
        use salvo::test::TestClient;

        let big_sld = format!("{SINGLESYMBOL_LINE_SLD}{}", " ".repeat(200_000));
        let service = Service::new(
            Router::with_path("convert-sld")
                .hoop(SecureMaxSize::new(8 * 1024 * 1024))
                .post(convert_sld),
        );
        let body = serde_json::json!({
            "sld": big_sld,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    }

    #[tokio::test]
    async fn convert_sld_without_secure_max_size_hoop_rejects_body_over_default_limit() {
        use salvo::test::TestClient;

        let big_sld = format!("{SINGLESYMBOL_LINE_SLD}{}", " ".repeat(200_000));
        let service = Service::new(Router::with_path("convert-sld").post(convert_sld));
        let body = serde_json::json!({
            "sld": big_sld,
            "source_layer": "test_layer",
            "mode": "qgis",
        });
        let resp = TestClient::post("http://127.0.0.1:5800/convert-sld")
            .json(&body)
            .send(&service)
            .await;

        assert!(resp.status_code.unwrap().is_client_error());
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(
            !content_type.contains("application/json"),
            "expected a non-JSON error body, got content-type: {content_type}"
        );
    }
}
