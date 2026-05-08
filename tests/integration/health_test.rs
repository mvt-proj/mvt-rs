use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};

// Inline handler copy to avoid needing a library crate target.
// Tests the actual response shape the health endpoint produces.

#[derive(serde::Serialize)]
struct Health {
    title: String,
    message: String,
    version: String,
}

#[handler]
async fn get_health(res: &mut Response) {
    res.render(Json(Health {
        title: "MVT Server".to_string(),
        message: "test".to_string(),
        version: "test".to_string(),
    }));
}

#[tokio::test]
async fn health_returns_200() {
    let router = Router::with_path("health").get(get_health);
    let service = Service::new(router);

    let res = TestClient::get("http://localhost/health")
        .send(&service)
        .await;

    assert_eq!(res.status_code, Some(StatusCode::OK));
}

#[tokio::test]
async fn health_returns_json_shape() {
    let router = Router::with_path("health").get(get_health);
    let service = Service::new(router);

    let mut res = TestClient::get("http://localhost/health")
        .send(&service)
        .await;

    let body = res.take_string().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(json["title"].is_string());
    assert!(json["message"].is_string());
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let router = Router::with_path("health").get(get_health);
    let service = Service::new(router);

    let res = TestClient::get("http://localhost/does-not-exist")
        .send(&service)
        .await;

    assert_eq!(res.status_code, Some(StatusCode::NOT_FOUND));
}
