use askama::Template;
use prometheus::{Encoder, TextEncoder};
use salvo::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use std::io;
use tokio::time::{Duration, interval};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::IntervalStream;

use crate::html::main::{BaseTemplateData, is_authenticated}; // Ajusta imports según tu proyecto
use crate::monitor::metrics::{
    AVG_LATENCY, CACHE_HITS, CACHE_MISSES, LAST_LATENCY, PROCESS_CPU, PROCESS_MEM, REGISTRY,
    REQUESTS_TOTAL,
};

#[derive(Template)]
#[template(path = "admin/monitor/dashboard.html")]
struct DashboardTemplate {
    base: BaseTemplateData,
}

#[handler]
pub async fn metrics(res: &mut Response) {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("failed to encode metrics: {e}");
    }
    let body = String::from_utf8(buffer).unwrap_or_else(|e| {
        tracing::error!("invalid UTF-8 in metrics: {e}");
        String::new()
    });

    res.headers_mut().insert(
        salvo::http::header::CONTENT_TYPE,
        salvo::http::HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    res.render(body);
}

#[handler]
pub async fn dashboard(res: &mut Response, depot: &mut Depot) {
    let is_auth = is_authenticated(depot).await;
    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();

    let base = BaseTemplateData { is_auth, translate };
    let template = DashboardTemplate { base };

    match template.render() {
        Ok(html) => res.render(Text::Html(html)),
        Err(e) => {
            tracing::error!("failed to render dashboard template: {e}");
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render("Template error");
        }
    }
}

#[handler]
pub async fn sse_metrics(res: &mut Response) {
    res.headers_mut().insert(
        salvo::http::header::CONTENT_TYPE,
        salvo::http::HeaderValue::from_static("text/event-stream"),
    );
    res.headers_mut().insert(
        salvo::http::header::CACHE_CONTROL,
        salvo::http::HeaderValue::from_static("no-cache"),
    );

    let tick = interval(Duration::from_secs(5));
    let stream = IntervalStream::new(tick).map(move |_| {
        let cpu = PROCESS_CPU.get();
        let mem_bytes = PROCESS_MEM.get();
        let mem_gb = mem_bytes / 1_000_000_000.0;
        let reqs = REQUESTS_TOTAL.get();
        let hits = CACHE_HITS.get();
        let misses = CACHE_MISSES.get();
        let last_lat = LAST_LATENCY.get();
        let avg_lat = AVG_LATENCY.get();

        let json = match serde_json::to_string(&json!({
            "cpu_percent": cpu,
            "memory_gb": mem_gb,
            "requests_total": reqs,
            "cache_hits_total": hits,
            "cache_misses_total": misses,
            "last_latency": last_lat,
            "avg_latency": avg_lat,
        })) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("failed to serialize SSE JSON: {e}");
                "{}".to_string()
            }
        };

        let msg = format!("data: {}\n\n", json);
        Ok::<String, io::Error>(msg)
    });

    res.stream(stream);
}
