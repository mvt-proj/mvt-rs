use prometheus::{Counter, Gauge, HistogramOpts, HistogramVec, Opts, Registry};
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

/// Per-layer, per-path (cache vs database) tile latency. Unlike `AVG_LATENCY`
/// (a single running average across all requests), this lets Prometheus/Grafana
/// compute real percentiles broken down by layer and by whether the tile came
/// from cache or the database.
pub static TILE_REQUEST_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    let opts = HistogramOpts::new(
        "tile_request_duration_seconds",
        "Tile request latency by layer and path (cache/database)",
    )
    .buckets(vec![
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ]);
    let hv = HistogramVec::new(opts, &["layer", "via"]).expect("invalid histogram opts");
    REGISTRY
        .register(Box::new(hv.clone()))
        .expect("metric registration failed");
    hv
});

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

/// Records a single tile request's latency under its layer and path
/// (`"cache"` or `"database"`). Backs the `tile_request_duration_seconds`
/// histogram, which `histogram_quantile()` can turn into real percentiles.
pub fn record_tile_latency(layer: &str, via: &str, secs: f64) {
    TILE_REQUEST_DURATION
        .with_label_values(&[layer, via])
        .observe(secs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_tile_latency_accumulates_count_and_sum_per_label_pair() {
        record_tile_latency("cat_roads", "cache", 0.01);
        record_tile_latency("cat_roads", "cache", 0.03);
        record_tile_latency("cat_roads", "database", 0.2);

        let cache_metric = TILE_REQUEST_DURATION.with_label_values(&["cat_roads", "cache"]);
        assert_eq!(cache_metric.get_sample_count(), 2);
        assert!((cache_metric.get_sample_sum() - 0.04).abs() < 1e-9);

        let db_metric = TILE_REQUEST_DURATION.with_label_values(&["cat_roads", "database"]);
        assert_eq!(db_metric.get_sample_count(), 1);
        assert!((db_metric.get_sample_sum() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn record_tile_latency_keeps_layers_separate() {
        record_tile_latency("cat_parks", "database", 0.5);

        let other_layer = TILE_REQUEST_DURATION.with_label_values(&["cat_parks", "cache"]);
        assert_eq!(other_layer.get_sample_count(), 0);
    }
}
