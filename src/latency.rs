//! Latency Tracking - First-Class Metric
//! 
//! Production-grade latency measurement for trading infrastructure.
//! Tracks exchange-to-client latency with percentile calculations.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Latency measurement for a single message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyMeasurement {
    /// Timestamp from exchange (when Kraken generated the message)
    pub exchange_timestamp: DateTime<Utc>,
    /// Timestamp when SDK received the message
    pub receive_timestamp: DateTime<Utc>,
    /// Timestamp when SDK finished processing
    pub process_timestamp: DateTime<Utc>,
    /// Network latency (exchange -> SDK receive)
    pub network_latency_us: i64,
    /// Processing latency (receive -> process complete)
    pub processing_latency_us: i64,
    /// Total end-to-end latency
    pub total_latency_us: i64,
    /// Channel/symbol this measurement is for
    pub channel: String,
    pub symbol: String,
}

/// Latency percentiles
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyPercentiles {
    pub p50: f64,  // Median
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub p999: f64, // Three nines
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub stddev: f64,
}

/// Histogram bucket for latency distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub range_start_us: i64,
    pub range_end_us: i64,
    pub count: u64,
    pub percentage: f64,
}

/// Latency histogram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyHistogram {
    pub buckets: Vec<HistogramBucket>,
    pub total_samples: u64,
    pub bucket_width_us: i64,
}

/// Comprehensive latency statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Network latency percentiles (exchange -> receive)
    pub network: LatencyPercentiles,
    /// Processing latency percentiles (receive -> done)
    pub processing: LatencyPercentiles,
    /// Total end-to-end latency percentiles
    pub total: LatencyPercentiles,
    /// Number of samples
    pub sample_count: u64,
    /// Samples per second
    pub samples_per_second: f64,
    /// Last measurement
    pub last_measurement: Option<LatencyMeasurement>,
    /// Histogram of total latency
    pub histogram: Option<LatencyHistogram>,
}

/// Configuration for latency tracker
#[derive(Debug, Clone)]
pub struct LatencyConfig {
    /// Maximum samples to keep for percentile calculation
    pub max_samples: usize,
    /// Histogram bucket width in microseconds
    pub histogram_bucket_us: i64,
    /// Number of histogram buckets
    pub histogram_buckets: usize,
    /// Window for samples_per_second calculation
    pub rate_window_secs: u64,
}

impl Default for LatencyConfig {
    fn default() -> Self {
        Self {
            max_samples: 10000,
            histogram_bucket_us: 1000, // 1ms buckets
            histogram_buckets: 100,    // Up to 100ms
            rate_window_secs: 10,
        }
    }
}

/// Callback for latency alerts
pub type LatencyAlertCallback = std::sync::Arc<dyn Fn(LatencyAlert) + Send + Sync>;

/// Latency alert event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyAlert {
    pub alert_type: LatencyAlertType,
    pub channel: String,
    pub symbol: String,
    pub latency_us: i64,
    pub threshold_us: i64,
    pub timestamp: DateTime<Utc>,
}

/// Types of latency alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LatencyAlertType {
    HighNetworkLatency,
    HighProcessingLatency,
    HighTotalLatency,
    LatencySpike { previous_us: i64 },
}

/// Production-grade latency tracker
pub struct LatencyTracker {
    config: LatencyConfig,
    /// Network latency samples (microseconds)
    network_samples: Mutex<VecDeque<i64>>,
    /// Processing latency samples (microseconds)
    processing_samples: Mutex<VecDeque<i64>>,
    /// Total latency samples (microseconds)
    total_samples: Mutex<VecDeque<i64>>,
    /// Recent measurements for rate calculation
    recent_timestamps: Mutex<VecDeque<Instant>>,
    /// Last measurement
    last_measurement: Mutex<Option<LatencyMeasurement>>,
    /// Alert callback
    alert_callback: Mutex<Option<LatencyAlertCallback>>,
    /// Alert thresholds (microseconds)
    network_threshold_us: Mutex<i64>,
    processing_threshold_us: Mutex<i64>,
    total_threshold_us: Mutex<i64>,
    /// Start time for uptime calculation
    start_time: Instant,
}

impl LatencyTracker {
    /// Create with default config
    pub fn new() -> Self {
        Self::with_config(LatencyConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: LatencyConfig) -> Self {
        Self {
            config,
            network_samples: Mutex::new(VecDeque::new()),
            processing_samples: Mutex::new(VecDeque::new()),
            total_samples: Mutex::new(VecDeque::new()),
            recent_timestamps: Mutex::new(VecDeque::new()),
            last_measurement: Mutex::new(None),
            alert_callback: Mutex::new(None),
            network_threshold_us: Mutex::new(100_000),    // 100ms default
            processing_threshold_us: Mutex::new(10_000),  // 10ms default
            total_threshold_us: Mutex::new(150_000),      // 150ms default
            start_time: Instant::now(),
        }
    }
    
    /// Set alert callback
    pub fn on_alert<F>(&self, callback: F) -> &Self
    where
        F: Fn(LatencyAlert) + Send + Sync + 'static
    {
        *self.alert_callback.lock().unwrap() = Some(std::sync::Arc::new(callback));
        self
    }
    
    /// Set alert thresholds (in microseconds)
    pub fn set_thresholds(&self, network_us: i64, processing_us: i64, total_us: i64) -> &Self {
        *self.network_threshold_us.lock().unwrap() = network_us;
        *self.processing_threshold_us.lock().unwrap() = processing_us;
        *self.total_threshold_us.lock().unwrap() = total_us;
        self
    }
    
    /// Record a latency measurement
    pub fn record(
        &self,
        exchange_timestamp: DateTime<Utc>,
        channel: &str,
        symbol: &str,
    ) -> LatencyMeasurement {
        let receive_timestamp = Utc::now();
        let process_start = Instant::now();
        
        // Calculate latencies
        let network_latency = receive_timestamp
            .signed_duration_since(exchange_timestamp)
            .num_microseconds()
            .unwrap_or(0);
        
        // Simulate minimal processing time
        let process_timestamp = Utc::now();
        let processing_latency = process_timestamp
            .signed_duration_since(receive_timestamp)
            .num_microseconds()
            .unwrap_or(0);
        
        let total_latency = network_latency + processing_latency;
        
        let measurement = LatencyMeasurement {
            exchange_timestamp,
            receive_timestamp,
            process_timestamp,
            network_latency_us: network_latency,
            processing_latency_us: processing_latency,
            total_latency_us: total_latency,
            channel: channel.to_string(),
            symbol: symbol.to_string(),
        };
        
        // Store samples
        self.add_sample(network_latency, processing_latency, total_latency);
        
        // Update last measurement
        *self.last_measurement.lock().unwrap() = Some(measurement.clone());
        
        // Record timestamp for rate calculation
        self.recent_timestamps.lock().unwrap().push_back(Instant::now());
        
        // Check for alerts
        self.check_alerts(&measurement);
        
        measurement
    }
    
    /// Record with explicit timestamps (for testing or replay)
    pub fn record_explicit(
        &self,
        exchange_timestamp: DateTime<Utc>,
        receive_timestamp: DateTime<Utc>,
        process_timestamp: DateTime<Utc>,
        channel: &str,
        symbol: &str,
    ) -> LatencyMeasurement {
        let network_latency = receive_timestamp
            .signed_duration_since(exchange_timestamp)
            .num_microseconds()
            .unwrap_or(0);
        
        let processing_latency = process_timestamp
            .signed_duration_since(receive_timestamp)
            .num_microseconds()
            .unwrap_or(0);
        
        let total_latency = network_latency + processing_latency;
        
        let measurement = LatencyMeasurement {
            exchange_timestamp,
            receive_timestamp,
            process_timestamp,
            network_latency_us: network_latency,
            processing_latency_us: processing_latency,
            total_latency_us: total_latency,
            channel: channel.to_string(),
            symbol: symbol.to_string(),
        };
        
        self.add_sample(network_latency, processing_latency, total_latency);
        *self.last_measurement.lock().unwrap() = Some(measurement.clone());
        self.recent_timestamps.lock().unwrap().push_back(Instant::now());
        self.check_alerts(&measurement);
        
        measurement
    }
    
    fn add_sample(&self, network: i64, processing: i64, total: i64) {
        let max = self.config.max_samples;
        
        let mut net = self.network_samples.lock().unwrap();
        net.push_back(network);
        while net.len() > max { net.pop_front(); }
        
        let mut proc = self.processing_samples.lock().unwrap();
        proc.push_back(processing);
        while proc.len() > max { proc.pop_front(); }
        
        let mut tot = self.total_samples.lock().unwrap();
        tot.push_back(total);
        while tot.len() > max { tot.pop_front(); }
    }
    
    fn check_alerts(&self, measurement: &LatencyMeasurement) {
        let callback = self.alert_callback.lock().unwrap();
        if callback.is_none() { return; }
        let cb = callback.as_ref().unwrap().clone();
        drop(callback);
        
        let net_threshold = *self.network_threshold_us.lock().unwrap();
        let proc_threshold = *self.processing_threshold_us.lock().unwrap();
        let total_threshold = *self.total_threshold_us.lock().unwrap();
        
        if measurement.network_latency_us > net_threshold {
            cb(LatencyAlert {
                alert_type: LatencyAlertType::HighNetworkLatency,
                channel: measurement.channel.clone(),
                symbol: measurement.symbol.clone(),
                latency_us: measurement.network_latency_us,
                threshold_us: net_threshold,
                timestamp: Utc::now(),
            });
        }
        
        if measurement.processing_latency_us > proc_threshold {
            cb(LatencyAlert {
                alert_type: LatencyAlertType::HighProcessingLatency,
                channel: measurement.channel.clone(),
                symbol: measurement.symbol.clone(),
                latency_us: measurement.processing_latency_us,
                threshold_us: proc_threshold,
                timestamp: Utc::now(),
            });
        }
        
        if measurement.total_latency_us > total_threshold {
            cb(LatencyAlert {
                alert_type: LatencyAlertType::HighTotalLatency,
                channel: measurement.channel.clone(),
                symbol: measurement.symbol.clone(),
                latency_us: measurement.total_latency_us,
                threshold_us: total_threshold,
                timestamp: Utc::now(),
            });
        }
    }
    
    /// Get comprehensive latency statistics
    pub fn stats(&self) -> LatencyStats {
        let network = self.calculate_percentiles(&self.network_samples.lock().unwrap());
        let processing = self.calculate_percentiles(&self.processing_samples.lock().unwrap());
        let total = self.calculate_percentiles(&self.total_samples.lock().unwrap());
        
        let sample_count = self.total_samples.lock().unwrap().len() as u64;
        
        // Calculate samples per second
        let mut recent = self.recent_timestamps.lock().unwrap();
        let now = Instant::now();
        let window = Duration::from_secs(self.config.rate_window_secs);
        while let Some(ts) = recent.front() {
            if now.duration_since(*ts) > window {
                recent.pop_front();
            } else {
                break;
            }
        }
        let samples_per_second = recent.len() as f64 / self.config.rate_window_secs as f64;
        
        // Build histogram
        let histogram = self.build_histogram();
        
        LatencyStats {
            network,
            processing,
            total,
            sample_count,
            samples_per_second,
            last_measurement: self.last_measurement.lock().unwrap().clone(),
            histogram: Some(histogram),
        }
    }
    
    fn calculate_percentiles(&self, samples: &VecDeque<i64>) -> LatencyPercentiles {
        if samples.is_empty() {
            return LatencyPercentiles::default();
        }
        
        let mut sorted: Vec<i64> = samples.iter().copied().collect();
        sorted.sort();
        
        let len = sorted.len();
        let sum: i64 = sorted.iter().sum();
        let mean = sum as f64 / len as f64;
        
        // Calculate standard deviation
        let variance: f64 = sorted.iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>() / len as f64;
        let stddev = variance.sqrt();
        
        LatencyPercentiles {
            p50: sorted[len * 50 / 100] as f64,
            p75: sorted[len * 75 / 100] as f64,
            p90: sorted[len * 90 / 100] as f64,
            p95: sorted[len * 95 / 100] as f64,
            p99: sorted[len * 99 / 100] as f64,
            p999: sorted[std::cmp::min(len * 999 / 1000, len - 1)] as f64,
            min: sorted[0] as f64,
            max: sorted[len - 1] as f64,
            mean,
            stddev,
        }
    }
    
    fn build_histogram(&self) -> LatencyHistogram {
        let samples = self.total_samples.lock().unwrap();
        let bucket_width = self.config.histogram_bucket_us;
        let num_buckets = self.config.histogram_buckets;
        
        let mut buckets: Vec<u64> = vec![0; num_buckets];
        
        for &sample in samples.iter() {
            let bucket_idx = (sample / bucket_width) as usize;
            let idx = std::cmp::min(bucket_idx, num_buckets - 1);
            buckets[idx] += 1;
        }
        
        let total = samples.len() as u64;
        let histogram_buckets: Vec<HistogramBucket> = buckets
            .iter()
            .enumerate()
            .map(|(i, &count)| HistogramBucket {
                range_start_us: i as i64 * bucket_width,
                range_end_us: (i as i64 + 1) * bucket_width,
                count,
                percentage: if total > 0 { count as f64 / total as f64 * 100.0 } else { 0.0 },
            })
            .collect();
        
        LatencyHistogram {
            buckets: histogram_buckets,
            total_samples: total,
            bucket_width_us: bucket_width,
        }
    }
    
    /// Get last measurement
    pub fn last(&self) -> Option<LatencyMeasurement> {
        self.last_measurement.lock().unwrap().clone()
    }
    
    /// Reset all samples
    pub fn reset(&self) {
        self.network_samples.lock().unwrap().clear();
        self.processing_samples.lock().unwrap().clear();
        self.total_samples.lock().unwrap().clear();
        self.recent_timestamps.lock().unwrap().clear();
        *self.last_measurement.lock().unwrap() = None;
    }
    
    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Format latency for display (microseconds to human readable)
pub fn format_latency(us: f64) -> String {
    if us < 1000.0 {
        format!("{:.0}µs", us)
    } else if us < 1_000_000.0 {
        format!("{:.2}ms", us / 1000.0)
    } else {
        format!("{:.2}s", us / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    
    #[test]
    fn test_basic_recording() {
        let tracker = LatencyTracker::new();
        let exchange_ts = Utc::now() - ChronoDuration::milliseconds(50);
        
        let measurement = tracker.record(exchange_ts, "ticker", "BTC/USD");
        
        assert!(measurement.network_latency_us > 0);
        assert!(measurement.total_latency_us >= measurement.network_latency_us);
    }
    
    #[test]
    fn test_percentiles() {
        let tracker = LatencyTracker::new();
        
        // Add known samples
        for i in 1..=100 {
            let exchange_ts = Utc::now() - ChronoDuration::microseconds(i * 1000);
            tracker.record(exchange_ts, "ticker", "BTC/USD");
        }
        
        let stats = tracker.stats();
        assert_eq!(stats.sample_count, 100);
        assert!(stats.total.p50 > 0.0);
        assert!(stats.total.p99 >= stats.total.p50);
    }
    
    #[test]
    fn test_histogram() {
        let tracker = LatencyTracker::new();
        
        for i in 1..=50 {
            let exchange_ts = Utc::now() - ChronoDuration::milliseconds(i);
            tracker.record(exchange_ts, "ticker", "BTC/USD");
        }
        
        let stats = tracker.stats();
        let histogram = stats.histogram.unwrap();
        
        assert!(histogram.total_samples > 0);
        assert!(!histogram.buckets.is_empty());
    }
    
    #[test]
    fn test_format_latency() {
        assert_eq!(format_latency(500.0), "500µs");
        assert_eq!(format_latency(1500.0), "1.50ms");
        assert_eq!(format_latency(1_500_000.0), "1.50s");
    }
}