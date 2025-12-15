//! Telemetry and observability for SDK operations
//!
//! Provides structured logging, metrics export, and distributed tracing support.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// SDK telemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Enable structured logging
    pub logging_enabled: bool,
    /// Enable distributed tracing
    pub tracing_enabled: bool,
    /// Metrics export interval
    pub export_interval: Duration,
    /// Service name for tracing
    pub service_name: String,
    /// Custom labels for metrics
    pub labels: HashMap<String, String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            logging_enabled: true,
            tracing_enabled: false,
            export_interval: Duration::from_secs(60),
            service_name: "kraken-ws-sdk".to_string(),
            labels: HashMap::new(),
        }
    }
}

impl TelemetryConfig {
    pub fn builder() -> TelemetryConfigBuilder {
        TelemetryConfigBuilder::new()
    }
}

pub struct TelemetryConfigBuilder {
    config: TelemetryConfig,
}

impl TelemetryConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: TelemetryConfig::default(),
        }
    }

    pub fn service_name(mut self, name: &str) -> Self {
        self.config.service_name = name.to_string();
        self
    }

    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = enabled;
        self
    }

    pub fn with_tracing(mut self, enabled: bool) -> Self {
        self.config.tracing_enabled = enabled;
        self
    }

    pub fn export_interval(mut self, interval: Duration) -> Self {
        self.config.export_interval = interval;
        self
    }

    pub fn label(mut self, key: &str, value: &str) -> Self {
        self.config.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn build(self) -> TelemetryConfig {
        self.config
    }
}

impl Default for TelemetryConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// SDK metrics registry
#[derive(Debug)]
pub struct MetricsRegistry {
    counters: std::sync::Mutex<HashMap<String, Counter>>,
    gauges: std::sync::Mutex<HashMap<String, Gauge>>,
    histograms: std::sync::Mutex<HashMap<String, Histogram>>,
    config: TelemetryConfig,
}

impl MetricsRegistry {
    pub fn new(config: TelemetryConfig) -> Self {
        Self {
            counters: std::sync::Mutex::new(HashMap::new()),
            gauges: std::sync::Mutex::new(HashMap::new()),
            histograms: std::sync::Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Get or create a counter
    pub fn counter(&self, name: &str, help: &str) -> Counter {
        let mut counters = self.counters.lock().unwrap();
        counters.entry(name.to_string())
            .or_insert_with(|| Counter::new(name, help))
            .clone()
    }

    /// Get or create a gauge
    pub fn gauge(&self, name: &str, help: &str) -> Gauge {
        let mut gauges = self.gauges.lock().unwrap();
        gauges.entry(name.to_string())
            .or_insert_with(|| Gauge::new(name, help))
            .clone()
    }

    /// Get or create a histogram
    pub fn histogram(&self, name: &str, help: &str, buckets: Vec<f64>) -> Histogram {
        let mut histograms = self.histograms.lock().unwrap();
        histograms.entry(name.to_string())
            .or_insert_with(|| Histogram::new(name, help, buckets))
            .clone()
    }

    /// Export all metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Export counters
        for counter in self.counters.lock().unwrap().values() {
            output.push_str(&counter.to_prometheus());
            output.push('\n');
        }
        
        // Export gauges
        for gauge in self.gauges.lock().unwrap().values() {
            output.push_str(&gauge.to_prometheus());
            output.push('\n');
        }
        
        // Export histograms
        for histogram in self.histograms.lock().unwrap().values() {
            output.push_str(&histogram.to_prometheus());
            output.push('\n');
        }
        
        output
    }
}

/// Counter metric
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    help: String,
    value: Arc<std::sync::atomic::AtomicU64>,
    labels: HashMap<String, String>,
}

impl Counter {
    pub fn new(name: &str, help: &str) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            value: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            labels: HashMap::new(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn inc_by(&self, n: u64) {
        self.value.fetch_add(n, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get(&self) -> u64 {
        self.value.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn to_prometheus(&self) -> String {
        let labels_str = if self.labels.is_empty() {
            String::new()
        } else {
            let pairs: Vec<String> = self.labels.iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect();
            format!("{{{}}}", pairs.join(","))
        };
        
        format!(
            "# HELP {} {}\n# TYPE {} counter\n{}{} {}",
            self.name, self.help, self.name, self.name, labels_str, self.get()
        )
    }
}

/// Gauge metric
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    help: String,
    value: Arc<std::sync::atomic::AtomicI64>,
    labels: HashMap<String, String>,
}

impl Gauge {
    pub fn new(name: &str, help: &str) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            value: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            labels: HashMap::new(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn set(&self, v: i64) {
        self.value.store(v, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn dec(&self) {
        self.value.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get(&self) -> i64 {
        self.value.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn to_prometheus(&self) -> String {
        let labels_str = if self.labels.is_empty() {
            String::new()
        } else {
            let pairs: Vec<String> = self.labels.iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect();
            format!("{{{}}}", pairs.join(","))
        };
        
        format!(
            "# HELP {} {}\n# TYPE {} gauge\n{}{} {}",
            self.name, self.help, self.name, self.name, labels_str, self.get()
        )
    }
}

/// Histogram metric
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    help: String,
    buckets: Vec<f64>,
    counts: Arc<std::sync::Mutex<Vec<u64>>>,
    sum: Arc<std::sync::atomic::AtomicU64>,
    count: Arc<std::sync::atomic::AtomicU64>,
}

impl Histogram {
    pub fn new(name: &str, help: &str, buckets: Vec<f64>) -> Self {
        let bucket_count = buckets.len();
        Self {
            name: name.to_string(),
            help: help.to_string(),
            buckets,
            counts: Arc::new(std::sync::Mutex::new(vec![0; bucket_count + 1])), // +1 for +Inf
            sum: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Default buckets for latency in milliseconds
    pub fn default_latency_buckets() -> Vec<f64> {
        vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0]
    }

    pub fn observe(&self, value: f64) {
        // Update sum and count
        self.sum.fetch_add((value * 1000.0) as u64, std::sync::atomic::Ordering::SeqCst);
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        // Update bucket counts
        let mut counts = self.counts.lock().unwrap();
        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                counts[i] += 1;
            }
        }
        // Always increment +Inf bucket
        *counts.last_mut().unwrap() += 1;
    }

    pub fn observe_duration(&self, duration: Duration) {
        self.observe(duration.as_secs_f64() * 1000.0); // Convert to ms
    }

    pub fn to_prometheus(&self) -> String {
        let counts = self.counts.lock().unwrap();
        let mut output = format!(
            "# HELP {} {}\n# TYPE {} histogram\n",
            self.name, self.help, self.name
        );
        
        let mut cumulative = 0u64;
        for (i, bucket) in self.buckets.iter().enumerate() {
            cumulative += counts[i];
            output.push_str(&format!(
                "{}_bucket{{le=\"{}\"}} {}\n",
                self.name, bucket, cumulative
            ));
        }
        
        // +Inf bucket
        cumulative += counts.last().unwrap();
        output.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", self.name, cumulative));
        
        // Sum and count
        let sum = self.sum.load(std::sync::atomic::Ordering::SeqCst) as f64 / 1000.0;
        let count = self.count.load(std::sync::atomic::Ordering::SeqCst);
        output.push_str(&format!("{}_sum {}\n", self.name, sum));
        output.push_str(&format!("{}_count {}", self.name, count));
        
        output
    }
}

/// Pre-configured SDK metrics
pub struct SdkMetrics {
    pub messages_received: Counter,
    pub messages_processed: Counter,
    pub messages_dropped: Counter,
    pub connection_attempts: Counter,
    pub connection_failures: Counter,
    pub reconnections: Counter,
    pub active_subscriptions: Gauge,
    pub connection_state: Gauge,
    pub message_latency: Histogram,
    pub processing_time: Histogram,
}

impl SdkMetrics {
    pub fn new(registry: &MetricsRegistry) -> Self {
        Self {
            messages_received: registry.counter(
                "kraken_sdk_messages_received_total",
                "Total messages received from WebSocket"
            ),
            messages_processed: registry.counter(
                "kraken_sdk_messages_processed_total",
                "Total messages successfully processed"
            ),
            messages_dropped: registry.counter(
                "kraken_sdk_messages_dropped_total",
                "Total messages dropped due to backpressure"
            ),
            connection_attempts: registry.counter(
                "kraken_sdk_connection_attempts_total",
                "Total connection attempts"
            ),
            connection_failures: registry.counter(
                "kraken_sdk_connection_failures_total",
                "Total connection failures"
            ),
            reconnections: registry.counter(
                "kraken_sdk_reconnections_total",
                "Total reconnection attempts"
            ),
            active_subscriptions: registry.gauge(
                "kraken_sdk_active_subscriptions",
                "Number of active subscriptions"
            ),
            connection_state: registry.gauge(
                "kraken_sdk_connection_state",
                "Current connection state (0=disconnected, 1=connecting, 2=connected)"
            ),
            message_latency: registry.histogram(
                "kraken_sdk_message_latency_ms",
                "Message latency from exchange to SDK in milliseconds",
                Histogram::default_latency_buckets()
            ),
            processing_time: registry.histogram(
                "kraken_sdk_processing_time_ms",
                "Time to process a message in milliseconds",
                vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0]
            ),
        }
    }
}

/// Span for distributed tracing
#[derive(Debug)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub tags: HashMap<String, String>,
    pub logs: Vec<SpanLog>,
}

#[derive(Debug, Clone)]
pub struct SpanLog {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

impl Span {
    pub fn new(operation_name: &str) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            span_id: uuid::Uuid::new_v4().to_string()[..16].to_string(),
            parent_span_id: None,
            operation_name: operation_name.to_string(),
            start_time: Instant::now(),
            end_time: None,
            tags: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn child(&self, operation_name: &str) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: uuid::Uuid::new_v4().to_string()[..16].to_string(),
            parent_span_id: Some(self.span_id.clone()),
            operation_name: operation_name.to_string(),
            start_time: Instant::now(),
            end_time: None,
            tags: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    pub fn log(&mut self, message: &str) {
        self.logs.push(SpanLog {
            timestamp: chrono::Utc::now(),
            message: message.to_string(),
        });
    }

    pub fn finish(&mut self) {
        self.end_time = Some(Instant::now());
    }

    pub fn duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.duration_since(self.start_time))
    }
}
