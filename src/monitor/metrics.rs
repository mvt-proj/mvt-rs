use prometheus::{Counter, Gauge, Opts, Registry};
use std::sync::LazyLock;

// Registro central
pub static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    Registry::new_custom(Some("mvt_server".into()), None).unwrap_or_else(|e| {
        tracing::error!("failed to create registry: {e}");
        Registry::new()
    })
});

// Definición de métricas
pub static PROCESS_CPU: LazyLock<Gauge> =
    LazyLock::new(|| register_gauge("process_cpu_percent", "CPU usage percent of this process"));

pub static PROCESS_MEM: LazyLock<Gauge> =
    LazyLock::new(|| register_gauge("process_memory_bytes", "Memory usage in bytes"));

pub static REQUESTS_TOTAL: LazyLock<Counter> =
    LazyLock::new(|| register_counter("requests_total", "Total number of HTTP requests"));

pub static CACHE_HITS: LazyLock<Counter> =
    LazyLock::new(|| register_counter("cache_hits_total", "Total cache hits"));

pub static CACHE_MISSES: LazyLock<Counter> =
    LazyLock::new(|| register_counter("cache_misses_total", "Total cache misses"));

pub static LAST_LATENCY: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge(
        "last_request_latency_seconds",
        "Latency of the last request",
    )
});

pub static AVG_LATENCY: LazyLock<Gauge> =
    LazyLock::new(|| register_gauge("avg_request_latency_seconds", "Average request latency"));

// Helpers privados para reducir boilerplate
fn register_gauge(name: &str, help: &str) -> Gauge {
    let g = Gauge::with_opts(Opts::new(name, help)).unwrap();
    REGISTRY
        .register(Box::new(g.clone()))
        .expect("metric registration failed");
    g
}

fn register_counter(name: &str, help: &str) -> Counter {
    let c = Counter::with_opts(Opts::new(name, help)).unwrap();
    REGISTRY
        .register(Box::new(c.clone()))
        .expect("metric registration failed");
    c
}

pub fn record_request() {
    REQUESTS_TOTAL.inc();
}

pub fn record_cache_hit() {
    CACHE_HITS.inc();
}

pub fn record_cache_miss() {
    CACHE_MISSES.inc();
}

pub fn record_latency(secs: f64) {
    LAST_LATENCY.set(secs);
    let current_avg = AVG_LATENCY.get();
    if current_avg == 0.0 {
        AVG_LATENCY.set(secs);
    } else {
        AVG_LATENCY.set((current_avg + secs) / 2.0);
    }
}
