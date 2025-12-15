//! Middleware/Interceptor system for SDK operations
//!
//! Allows developers to hook into the request/response pipeline for:
//! - Logging
//! - Metrics collection
//! - Request modification
//! - Response transformation
//! - Error handling

use crate::error::SdkError;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Context passed through the middleware chain
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique request ID for correlation
    pub request_id: String,
    /// Operation name
    pub operation: String,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Custom metadata
    pub metadata: std::collections::HashMap<String, String>,
    /// Start time for latency tracking
    pub start_time: Instant,
}

impl RequestContext {
    pub fn new(operation: &str) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            operation: operation.to_string(),
            timestamp: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            start_time: Instant::now(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Response context with timing and metadata
#[derive(Debug, Clone)]
pub struct ResponseContext {
    pub request_id: String,
    pub operation: String,
    pub latency: Duration,
    pub success: bool,
    pub error: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl ResponseContext {
    pub fn success(ctx: &RequestContext) -> Self {
        Self {
            request_id: ctx.request_id.clone(),
            operation: ctx.operation.clone(),
            latency: ctx.elapsed(),
            success: true,
            error: None,
            metadata: ctx.metadata.clone(),
        }
    }

    pub fn failure(ctx: &RequestContext, error: &str) -> Self {
        Self {
            request_id: ctx.request_id.clone(),
            operation: ctx.operation.clone(),
            latency: ctx.elapsed(),
            success: false,
            error: Some(error.to_string()),
            metadata: ctx.metadata.clone(),
        }
    }
}

/// Middleware trait for intercepting SDK operations
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before an operation is executed
    async fn before(&self, ctx: &mut RequestContext) -> Result<(), SdkError>;
    
    /// Called after an operation completes (success or failure)
    async fn after(&self, ctx: &ResponseContext);
    
    /// Middleware name for debugging
    fn name(&self) -> &str;
}

/// Logging middleware - logs all operations
pub struct LoggingMiddleware {
    level: tracing::Level,
}

impl LoggingMiddleware {
    pub fn new(level: tracing::Level) -> Self {
        Self { level }
    }

    pub fn debug() -> Self {
        Self::new(tracing::Level::DEBUG)
    }

    pub fn info() -> Self {
        Self::new(tracing::Level::INFO)
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before(&self, ctx: &mut RequestContext) -> Result<(), SdkError> {
        tracing::info!(
            request_id = %ctx.request_id,
            operation = %ctx.operation,
            "Starting operation"
        );
        Ok(())
    }

    async fn after(&self, ctx: &ResponseContext) {
        if ctx.success {
            tracing::info!(
                request_id = %ctx.request_id,
                operation = %ctx.operation,
                latency_ms = %ctx.latency.as_millis(),
                "Operation completed successfully"
            );
        } else {
            tracing::warn!(
                request_id = %ctx.request_id,
                operation = %ctx.operation,
                latency_ms = %ctx.latency.as_millis(),
                error = ?ctx.error,
                "Operation failed"
            );
        }
    }

    fn name(&self) -> &str {
        "LoggingMiddleware"
    }
}

/// Metrics middleware - collects operation metrics
pub struct MetricsMiddleware {
    metrics: Arc<OperationMetrics>,
}

impl MetricsMiddleware {
    pub fn new(metrics: Arc<OperationMetrics>) -> Self {
        Self { metrics }
    }
}

#[async_trait]
impl Middleware for MetricsMiddleware {
    async fn before(&self, _ctx: &mut RequestContext) -> Result<(), SdkError> {
        self.metrics.increment_in_flight();
        Ok(())
    }

    async fn after(&self, ctx: &ResponseContext) {
        self.metrics.decrement_in_flight();
        self.metrics.record_latency(&ctx.operation, ctx.latency);
        
        if ctx.success {
            self.metrics.increment_success(&ctx.operation);
        } else {
            self.metrics.increment_failure(&ctx.operation);
        }
    }

    fn name(&self) -> &str {
        "MetricsMiddleware"
    }
}

/// Operation metrics collector
#[derive(Debug, Default)]
pub struct OperationMetrics {
    in_flight: std::sync::atomic::AtomicU64,
    total_requests: std::sync::atomic::AtomicU64,
    successful_requests: std::sync::atomic::AtomicU64,
    failed_requests: std::sync::atomic::AtomicU64,
    latencies: std::sync::Mutex<Vec<(String, Duration)>>,
}

impl OperationMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment_in_flight(&self) {
        self.in_flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.total_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn decrement_in_flight(&self) {
        self.in_flight.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn increment_success(&self, _operation: &str) {
        self.successful_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn increment_failure(&self, _operation: &str) {
        self.failed_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn record_latency(&self, operation: &str, latency: Duration) {
        if let Ok(mut latencies) = self.latencies.lock() {
            latencies.push((operation.to_string(), latency));
            // Keep only last 1000 samples
            if latencies.len() > 1000 {
                latencies.remove(0);
            }
        }
    }

    pub fn get_stats(&self) -> MetricsSnapshot {
        let latencies = self.latencies.lock().unwrap();
        let latency_values: Vec<u64> = latencies.iter().map(|(_, d)| d.as_micros() as u64).collect();
        
        MetricsSnapshot {
            in_flight: self.in_flight.load(std::sync::atomic::Ordering::SeqCst),
            total_requests: self.total_requests.load(std::sync::atomic::Ordering::SeqCst),
            successful_requests: self.successful_requests.load(std::sync::atomic::Ordering::SeqCst),
            failed_requests: self.failed_requests.load(std::sync::atomic::Ordering::SeqCst),
            avg_latency_us: if latency_values.is_empty() { 0 } else { 
                latency_values.iter().sum::<u64>() / latency_values.len() as u64 
            },
            p99_latency_us: percentile(&latency_values, 99),
        }
    }
}

fn percentile(values: &[u64], p: u8) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let idx = (sorted.len() as f64 * p as f64 / 100.0) as usize;
    sorted.get(idx.min(sorted.len() - 1)).copied().unwrap_or(0)
}

/// Metrics snapshot for export
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub in_flight: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_us: u64,
    pub p99_latency_us: u64,
}

impl MetricsSnapshot {
    /// Export as Prometheus format
    pub fn to_prometheus(&self, prefix: &str) -> String {
        format!(
            r#"# HELP {prefix}_requests_in_flight Current in-flight requests
# TYPE {prefix}_requests_in_flight gauge
{prefix}_requests_in_flight {in_flight}

# HELP {prefix}_requests_total Total requests
# TYPE {prefix}_requests_total counter
{prefix}_requests_total {total}

# HELP {prefix}_requests_success_total Successful requests
# TYPE {prefix}_requests_success_total counter
{prefix}_requests_success_total {success}

# HELP {prefix}_requests_failure_total Failed requests
# TYPE {prefix}_requests_failure_total counter
{prefix}_requests_failure_total {failure}

# HELP {prefix}_latency_avg_microseconds Average latency
# TYPE {prefix}_latency_avg_microseconds gauge
{prefix}_latency_avg_microseconds {avg_latency}

# HELP {prefix}_latency_p99_microseconds P99 latency
# TYPE {prefix}_latency_p99_microseconds gauge
{prefix}_latency_p99_microseconds {p99_latency}
"#,
            prefix = prefix,
            in_flight = self.in_flight,
            total = self.total_requests,
            success = self.successful_requests,
            failure = self.failed_requests,
            avg_latency = self.avg_latency_us,
            p99_latency = self.p99_latency_us,
        )
    }
}

/// Rate limiting middleware
pub struct RateLimitMiddleware {
    requests_per_second: u32,
    last_request: std::sync::Mutex<Instant>,
    request_count: std::sync::atomic::AtomicU32,
}

impl RateLimitMiddleware {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            requests_per_second,
            last_request: std::sync::Mutex::new(Instant::now()),
            request_count: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl Middleware for RateLimitMiddleware {
    async fn before(&self, ctx: &mut RequestContext) -> Result<(), SdkError> {
        let mut last = self.last_request.lock().unwrap();
        let elapsed = last.elapsed();
        
        if elapsed >= Duration::from_secs(1) {
            // Reset counter every second
            self.request_count.store(0, std::sync::atomic::Ordering::SeqCst);
            *last = Instant::now();
        }
        
        let count = self.request_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        if count >= self.requests_per_second {
            ctx.metadata.insert("rate_limited".to_string(), "true".to_string());
            return Err(SdkError::Network("Rate limit exceeded".to_string()));
        }
        
        Ok(())
    }

    async fn after(&self, _ctx: &ResponseContext) {}

    fn name(&self) -> &str {
        "RateLimitMiddleware"
    }
}

/// Middleware chain executor
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn add<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    pub async fn execute_before(&self, ctx: &mut RequestContext) -> Result<(), SdkError> {
        for middleware in &self.middlewares {
            middleware.before(ctx).await?;
        }
        Ok(())
    }

    pub async fn execute_after(&self, ctx: &ResponseContext) {
        // Execute in reverse order
        for middleware in self.middlewares.iter().rev() {
            middleware.after(ctx).await;
        }
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}
