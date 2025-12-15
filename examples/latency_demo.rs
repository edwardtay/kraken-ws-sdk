//! Latency Tracking Demo
//! 
//! Demonstrates production-grade latency measurement with:
//! - Exchange-to-client latency tracking
//! - Percentile calculations (p50, p95, p99)
//! - Histogram visualization
//! - High latency alerts

use chrono::{Duration as ChronoDuration, Utc};
use kraken_ws_sdk::{
    LatencyTracker, LatencyConfig, LatencyAlertType, format_latency,
};
use std::thread;
use std::time::Duration;

fn main() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  🕐 Kraken SDK - Latency Tracking Demo");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Create tracker with custom config
    let config = LatencyConfig {
        max_samples: 1000,
        histogram_bucket_us: 5000,  // 5ms buckets
        histogram_buckets: 20,      // Up to 100ms
        rate_window_secs: 5,
    };
    
    let tracker = LatencyTracker::with_config(config);
    
    // Set up alert callback
    tracker.on_alert(|alert| {
        let alert_type = match alert.alert_type {
            LatencyAlertType::HighNetworkLatency => "🌐 HIGH NETWORK",
            LatencyAlertType::HighProcessingLatency => "⚙️ HIGH PROCESSING",
            LatencyAlertType::HighTotalLatency => "🚨 HIGH TOTAL",
            LatencyAlertType::LatencySpike { .. } => "📈 SPIKE",
        };
        println!("{} ALERT: {} {} - {}µs (threshold: {}µs)",
            alert_type, alert.channel, alert.symbol,
            alert.latency_us, alert.threshold_us);
    });
    
    // Set thresholds (in microseconds)
    tracker.set_thresholds(
        50_000,   // 50ms network threshold
        5_000,    // 5ms processing threshold
        60_000,   // 60ms total threshold
    );
    
    println!("📊 Simulating latency measurements...\n");
    
    // Simulate various latency scenarios
    let scenarios = [
        ("ticker", "BTC/USD", 15),   // 15ms typical
        ("ticker", "ETH/USD", 20),   // 20ms typical
        ("book", "BTC/USD", 25),     // 25ms for orderbook
        ("trades", "BTC/USD", 10),   // 10ms for trades
    ];
    
    // Generate measurements
    for i in 0..100 {
        for (channel, symbol, base_latency_ms) in &scenarios {
            // Add some variance
            let variance = (i % 10) as i64 - 5;
            let latency_ms = *base_latency_ms as i64 + variance;
            
            // Simulate exchange timestamp (in the past)
            let exchange_ts = Utc::now() - ChronoDuration::milliseconds(latency_ms);
            
            let measurement = tracker.record(exchange_ts, channel, symbol);
            
            if i == 0 {
                println!("Sample measurement for {}/{}:", channel, symbol);
                println!("  Exchange timestamp: {}", measurement.exchange_timestamp);
                println!("  Receive timestamp:  {}", measurement.receive_timestamp);
                println!("  Network latency:    {}", format_latency(measurement.network_latency_us as f64));
                println!("  Processing latency: {}", format_latency(measurement.processing_latency_us as f64));
                println!("  Total latency:      {}", format_latency(measurement.total_latency_us as f64));
                println!();
            }
        }
        
        // Small delay between batches
        if i % 20 == 0 {
            thread::sleep(Duration::from_millis(10));
        }
    }
    
    // Simulate a high latency spike
    println!("⚡ Simulating high latency spike...\n");
    let spike_ts = Utc::now() - ChronoDuration::milliseconds(75);
    tracker.record(spike_ts, "ticker", "BTC/USD");
    
    // Get statistics
    let stats = tracker.stats();
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  📈 Latency Statistics");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    
    println!("Sample count: {}", stats.sample_count);
    println!("Samples/sec:  {:.1}", stats.samples_per_second);
    println!();
    
    println!("🌐 Network Latency:");
    println!("  Min:    {}", format_latency(stats.network.min));
    println!("  p50:    {}", format_latency(stats.network.p50));
    println!("  p95:    {}", format_latency(stats.network.p95));
    println!("  p99:    {}", format_latency(stats.network.p99));
    println!("  Max:    {}", format_latency(stats.network.max));
    println!("  Mean:   {}", format_latency(stats.network.mean));
    println!("  StdDev: {}", format_latency(stats.network.stddev));
    println!();
    
    println!("⚙️ Processing Latency:");
    println!("  Min:    {}", format_latency(stats.processing.min));
    println!("  p50:    {}", format_latency(stats.processing.p50));
    println!("  p95:    {}", format_latency(stats.processing.p95));
    println!("  p99:    {}", format_latency(stats.processing.p99));
    println!("  Max:    {}", format_latency(stats.processing.max));
    println!();
    
    println!("📊 Total End-to-End Latency:");
    println!("  Min:    {}", format_latency(stats.total.min));
    println!("  p50:    {}", format_latency(stats.total.p50));
    println!("  p95:    {}", format_latency(stats.total.p95));
    println!("  p99:    {}", format_latency(stats.total.p99));
    println!("  p99.9:  {}", format_latency(stats.total.p999));
    println!("  Max:    {}", format_latency(stats.total.max));
    println!();
    
    // Print histogram
    if let Some(histogram) = &stats.histogram {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  📊 Latency Histogram");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        
        let max_count = histogram.buckets.iter().map(|b| b.count).max().unwrap_or(1);
        
        for bucket in &histogram.buckets {
            if bucket.count > 0 {
                let bar_width = (bucket.count as f64 / max_count as f64 * 40.0) as usize;
                let bar = "█".repeat(bar_width);
                println!("{:>8} - {:>8}: {:>5} ({:>5.1}%) {}",
                    format_latency(bucket.range_start_us as f64),
                    format_latency(bucket.range_end_us as f64),
                    bucket.count,
                    bucket.percentage,
                    bar);
            }
        }
    }
    
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  ✅ Latency tracking demo complete!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
