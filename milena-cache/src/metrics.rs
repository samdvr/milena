use prometheus::{Counter, Histogram, IntCounter, Registry};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,
    pub request_counter: Counter,
    pub error_counter: Counter,
    pub operation_duration: Histogram,
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,
}

impl Metrics {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let registry = Registry::new();

        let request_counter =
            Counter::new("cache_requests_total", "Total number of cache requests")?;
        registry.register(Box::new(request_counter.clone()))?;

        let error_counter = Counter::new("cache_errors_total", "Total number of cache errors")?;
        registry.register(Box::new(error_counter.clone()))?;

        let operation_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "cache_operation_duration_seconds",
                "Duration of cache operations",
            )
            .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 2.0, 5.0]),
        )?;
        registry.register(Box::new(operation_duration.clone()))?;

        let cache_hits = IntCounter::new("cache_hits_total", "Total number of cache hits")?;
        registry.register(Box::new(cache_hits.clone()))?;

        let cache_misses = IntCounter::new("cache_misses_total", "Total number of cache misses")?;
        registry.register(Box::new(cache_misses.clone()))?;

        Ok(Self {
            registry: Arc::new(registry),
            request_counter,
            error_counter,
            operation_duration,
            cache_hits,
            cache_misses,
        })
    }
}
