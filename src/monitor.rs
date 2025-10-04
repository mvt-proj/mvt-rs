use askama::Template;
use std::collections::HashMap;
use salvo::prelude::*;
use serde_json::json;
use std::io;

use prometheus::{Counter, Encoder, Gauge, Opts, Registry, TextEncoder};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::time::{Duration, interval};
use tokio_stream::{StreamExt, wrappers::IntervalStream};
use crate::html::main::{BaseTemplateData, is_authenticated};
use std::time::Instant;

#[derive(Template)]
#[template(path = "admin/monitor/dashboard.html")]
struct DashboardTemplate {
    base: BaseTemplateData,
}

pub static REGISTRY: LazyLock<Registry> =
    LazyLock::new(|| Registry::new_custom(Some("vectiles".into()), None)
        .unwrap_or_else(|e| {
            tracing::error!("failed to create registry: {e}");
            Registry::new()
        }));

pub static PROCESS_CPU: LazyLock<Gauge> = LazyLock::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "process_cpu_percent",
        "CPU usage percent of this process",
    )).unwrap_or_else(|e| {
        tracing::error!("failed to create PROCESS_CPU gauge: {e}");
        Gauge::new("dummy_cpu", "dummy").unwrap()
    });
    if let Err(e) = REGISTRY.register(Box::new(g.clone())) {
        tracing::warn!("failed to register PROCESS_CPU: {e}");
    }
    g
});

pub static PROCESS_MEM: LazyLock<Gauge> = LazyLock::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "process_memory_bytes",
        "Resident memory (bytes) of this process",
    )).unwrap_or_else(|e| {
        tracing::error!("failed to create PROCESS_MEM gauge: {e}");
        Gauge::new("dummy_mem", "dummy").unwrap()
    });
    if let Err(e) = REGISTRY.register(Box::new(g.clone())) {
        tracing::warn!("failed to register PROCESS_MEM: {e}");
    }
    g
});

pub static REQUESTS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    let c = Counter::with_opts(Opts::new("requests_total", "Total HTTP requests served"))
        .unwrap_or_else(|e| {
            tracing::error!("failed to create REQUESTS_TOTAL counter: {e}");
            Counter::new("dummy_requests", "dummy").unwrap()
        });
    if let Err(e) = REGISTRY.register(Box::new(c.clone())) {
        tracing::warn!("failed to register REQUESTS_TOTAL: {e}");
    }
    c
});

pub static CACHE_HITS: LazyLock<Counter> = LazyLock::new(|| {
    let c = Counter::with_opts(Opts::new("cache_hits_total", "Total cache hits"))
        .unwrap_or_else(|e| {
            tracing::error!("failed to create CACHE_HITS counter: {e}");
            Counter::new("dummy_hits", "dummy").unwrap()
        });
    if let Err(e) = REGISTRY.register(Box::new(c.clone())) {
        tracing::warn!("failed to register CACHE_HITS: {e}");
    }
    c
});

pub static CACHE_MISSES: LazyLock<Counter> = LazyLock::new(|| {
    let c = Counter::with_opts(Opts::new("cache_misses_total", "Total cache misses"))
        .unwrap_or_else(|e| {
            tracing::error!("failed to create CACHE_MISSES counter: {e}");
            Counter::new("dummy_misses", "dummy").unwrap()
        });
    if let Err(e) = REGISTRY.register(Box::new(c.clone())) {
        tracing::warn!("failed to register CACHE_MISSES: {e}");
    }
    c
});

pub static LAST_LATENCY: LazyLock<Gauge> = LazyLock::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "request_latency_seconds",
        "Last request latency in seconds",
    )).unwrap_or_else(|e| {
        tracing::error!("failed to create LAST_LATENCY gauge: {e}");
        Gauge::new("dummy_last_latency", "dummy").unwrap()
    });
    if let Err(e) = REGISTRY.register(Box::new(g.clone())) {
        tracing::warn!("failed to register LAST_LATENCY: {e}");
    }
    g
});

pub static AVG_LATENCY: LazyLock<Gauge> = LazyLock::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "request_latency_avg_seconds",
        "Average request latency in seconds",
    )).unwrap_or_else(|e| {
        tracing::error!("failed to create AVG_LATENCY gauge: {e}");
        Gauge::new("dummy_avg_latency", "dummy").unwrap()
    });
    if let Err(e) = REGISTRY.register(Box::new(g.clone())) {
        tracing::warn!("failed to register AVG_LATENCY: {e}");
    }
    g
});

pub static LAT_SUM: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
pub static LAT_COUNT: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

pub fn record_latency(latency_secs: f64) {
    let nanos = (latency_secs * 1e9) as u64;
    LAT_SUM.fetch_add(nanos, Ordering::Relaxed);
    LAT_COUNT.fetch_add(1, Ordering::Relaxed);
    LAST_LATENCY.set(latency_secs);
}

pub fn update_avg_latency() {
    let sum = LAT_SUM.load(Ordering::Relaxed);
    let count = LAT_COUNT.load(Ordering::Relaxed);
    if count > 0 {
        let avg_secs = (sum as f64) / (count as f64) / 1e9;
        AVG_LATENCY.set(avg_secs);
    }
}

#[cfg(unix)]
fn get_cpu_time() -> Option<(f64, f64)> {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1_000_000.0;
            let system = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1_000_000.0;
            Some((user, system))
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn get_cpu_time() -> Option<(f64, f64)> {
    // Windows no soporta getrusage, retornar None para que use sysinfo
    None
}

pub fn spawn_updater() {
    tokio::spawn(async {
        let pid_num = std::process::id() as usize;
        let pid = Pid::from(pid_num);

        let mut sys = System::new_all();
        let mut intv = interval(Duration::from_secs(10));

        let mut last_cpu_time: Option<(f64, f64)> = None;
        let mut last_wall_time = Instant::now();
        let mut warned_once = false;

        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

        loop {
            intv.tick().await;

            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

            if let Some(p) = sys.process(pid) {
                let mem_bytes = p.memory();
                PROCESS_MEM.set(mem_bytes as f64);

                let cpu_from_sysinfo = p.cpu_usage() as f64;

                if cpu_from_sysinfo > 0.01 {
                    PROCESS_CPU.set(cpu_from_sysinfo);
                } else {
                    #[cfg(unix)]
                    {
                        if let Some((user, system)) = get_cpu_time() {
                            if let Some((last_user, last_system)) = last_cpu_time {
                                let now = Instant::now();
                                let wall_elapsed = now.duration_since(last_wall_time).as_secs_f64();

                                if wall_elapsed > 0.0 {
                                    let user_delta = user - last_user;
                                    let system_delta = system - last_system;
                                    let cpu_delta = user_delta + system_delta;

                                    let cpu_percent = (cpu_delta / wall_elapsed) * 100.0;

                                    PROCESS_CPU.set(cpu_percent.clamp(0.0, 100.0));
                                }

                                last_wall_time = now;
                            }
                            last_cpu_time = Some((user, system));
                        } else if !warned_once {
                            tracing::warn!("CPU metrics unavailable (jail/container restriction)");
                            warned_once = true;
                            PROCESS_CPU.set(0.0);
                        }
                    }

                    #[cfg(windows)]
                    {
                        if !warned_once {
                            tracing::warn!("CPU metrics from sysinfo returned 0 - this may be normal on Windows");
                            warned_once = true;
                        }
                        PROCESS_CPU.set(0.0);
                    }
                }
            }

            update_avg_latency();
        }
    });
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
