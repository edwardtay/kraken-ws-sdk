//! Whale Order Detection for Professional Trading Visualization
//!
//! Detects statistical outliers in order book liquidity - orders that are
//! significantly larger than the rolling average. These "whale" orders often
//! signal institutional activity or major market moves.
//!
//! ## Example: Detecting Whale Orders
//!
//! ```rust,ignore
//! use kraken_ws_sdk::extended::advanced::{WhaleDetector, WhaleConfig};
//!
//! let detector = WhaleDetector::new(WhaleConfig {
//!     window_size: 100,           // Rolling window of 100 observations
//!     outlier_threshold: 2.5,     // 2.5 standard deviations = whale
//!     min_absolute_size: dec!(5), // Ignore tiny orders even if outliers
//! });
//!
//! // On each order book update
//! let whales = detector.analyze(&order_book);
//!
//! for whale in whales {
//!     println!("🐋 WHALE @ {}: {} (z-score: {:.2})",
//!         whale.price, whale.volume, whale.z_score);
//! }
//! ```

use crate::orderbook::OrderBook;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIGURATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Configuration for whale order detection
#[derive(Debug, Clone)]
pub struct WhaleConfig {
    /// Number of observations in the rolling window for statistics
    pub window_size: usize,
    /// Z-score threshold for outlier detection (2.5 = top ~0.6%)
    pub outlier_threshold: f64,
    /// Minimum absolute size to consider (filters noise)
    pub min_absolute_size: Decimal,
    /// Number of price levels to analyze from best bid/ask
    pub analyze_depth: usize,
}

impl Default for WhaleConfig {
    fn default() -> Self {
        Self {
            window_size: 100,
            outlier_threshold: 2.5,
            min_absolute_size: Decimal::from(1),
            analyze_depth: 25,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DATA STRUCTURES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A detected whale order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleDetection {
    /// Symbol this whale was detected in
    pub symbol: String,
    /// Price level of the whale order
    pub price: Decimal,
    /// Volume of the whale order
    pub volume: Decimal,
    /// Side of the order book
    pub side: WhaleSide,
    /// Z-score: how many standard deviations above mean
    pub z_score: f64,
    /// Rolling average at time of detection
    pub rolling_avg: Decimal,
    /// Rolling standard deviation at time of detection
    pub rolling_stddev: Decimal,
    /// Timestamp of detection
    pub timestamp: DateTime<Utc>,
}

/// Side of the whale order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhaleSide {
    Bid,
    Ask,
}

/// Rolling statistics for a symbol
#[derive(Debug, Default)]
struct RollingStats {
    /// Recent volume observations
    observations: VecDeque<f64>,
    /// Running sum for efficient mean calculation
    sum: f64,
    /// Running sum of squares for efficient variance calculation
    sum_sq: f64,
}

impl RollingStats {
    fn new(capacity: usize) -> Self {
        Self {
            observations: VecDeque::with_capacity(capacity),
            sum: 0.0,
            sum_sq: 0.0,
        }
    }
    
    fn add(&mut self, value: f64, max_size: usize) {
        // Remove oldest if at capacity
        if self.observations.len() >= max_size {
            if let Some(old) = self.observations.pop_front() {
                self.sum -= old;
                self.sum_sq -= old * old;
            }
        }
        
        // Add new observation
        self.observations.push_back(value);
        self.sum += value;
        self.sum_sq += value * value;
    }
    
    fn mean(&self) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        self.sum / self.observations.len() as f64
    }
    
    fn stddev(&self) -> f64 {
        let n = self.observations.len() as f64;
        if n < 2.0 {
            return 0.0;
        }
        
        let mean = self.mean();
        let variance = (self.sum_sq / n) - (mean * mean);
        
        // Handle floating point errors
        if variance < 0.0 {
            return 0.0;
        }
        
        variance.sqrt()
    }
    
    fn z_score(&self, value: f64) -> f64 {
        let stddev = self.stddev();
        if stddev == 0.0 {
            return 0.0;
        }
        (value - self.mean()) / stddev
    }
    
    fn count(&self) -> usize {
        self.observations.len()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// WHALE DETECTOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Statistical whale order detector
///
/// Maintains rolling statistics per symbol and detects orders that are
/// statistical outliers (significantly larger than typical orders).
pub struct WhaleDetector {
    config: WhaleConfig,
    /// Rolling statistics by symbol
    stats: Mutex<HashMap<String, RollingStats>>,
}

impl WhaleDetector {
    /// Create a new whale detector with default config
    pub fn new() -> Self {
        Self::with_config(WhaleConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: WhaleConfig) -> Self {
        Self {
            config,
            stats: Mutex::new(HashMap::new()),
        }
    }
    
    /// Analyze an order book for whale orders
    ///
    /// Returns a list of detected whales, sorted by z-score (largest first).
    pub fn analyze(&self, book: &OrderBook) -> Vec<WhaleDetection> {
        let mut whales = Vec::new();
        let mut stats = self.stats.lock().unwrap();
        
        let symbol_stats = stats
            .entry(book.symbol.clone())
            .or_insert_with(|| RollingStats::new(self.config.window_size));
        
        let now = Utc::now();
        let min_size_f64 = decimal_to_f64(self.config.min_absolute_size);
        
        // Analyze bid side
        for (price, level) in book.bids.iter().rev().take(self.config.analyze_depth) {
            let volume_f64 = decimal_to_f64(level.volume);
            
            // Update rolling stats
            symbol_stats.add(volume_f64, self.config.window_size);
            
            // Check if whale (only if we have enough data)
            if symbol_stats.count() >= 10 && volume_f64 >= min_size_f64 {
                let z = symbol_stats.z_score(volume_f64);
                
                if z >= self.config.outlier_threshold {
                    whales.push(WhaleDetection {
                        symbol: book.symbol.clone(),
                        price: *price,
                        volume: level.volume,
                        side: WhaleSide::Bid,
                        z_score: z,
                        rolling_avg: f64_to_decimal(symbol_stats.mean()),
                        rolling_stddev: f64_to_decimal(symbol_stats.stddev()),
                        timestamp: now,
                    });
                }
            }
        }
        
        // Analyze ask side
        for (price, level) in book.asks.iter().take(self.config.analyze_depth) {
            let volume_f64 = decimal_to_f64(level.volume);
            
            // Update rolling stats
            symbol_stats.add(volume_f64, self.config.window_size);
            
            // Check if whale
            if symbol_stats.count() >= 10 && volume_f64 >= min_size_f64 {
                let z = symbol_stats.z_score(volume_f64);
                
                if z >= self.config.outlier_threshold {
                    whales.push(WhaleDetection {
                        symbol: book.symbol.clone(),
                        price: *price,
                        volume: level.volume,
                        side: WhaleSide::Ask,
                        z_score: z,
                        rolling_avg: f64_to_decimal(symbol_stats.mean()),
                        rolling_stddev: f64_to_decimal(symbol_stats.stddev()),
                        timestamp: now,
                    });
                }
            }
        }
        
        // Sort by z-score (largest outliers first)
        whales.sort_by(|a, b| b.z_score.partial_cmp(&a.z_score).unwrap_or(std::cmp::Ordering::Equal));
        
        whales
    }
    
    /// Get current statistics for a symbol
    pub fn get_stats(&self, symbol: &str) -> Option<(Decimal, Decimal)> {
        let stats = self.stats.lock().unwrap();
        stats.get(symbol).map(|s| {
            (f64_to_decimal(s.mean()), f64_to_decimal(s.stddev()))
        })
    }
    
    /// Reset statistics for a symbol
    pub fn reset(&self, symbol: &str) {
        let mut stats = self.stats.lock().unwrap();
        stats.remove(symbol);
    }
    
    /// Reset all statistics
    pub fn reset_all(&self) {
        let mut stats = self.stats.lock().unwrap();
        stats.clear();
    }
}

impl Default for WhaleDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HELPERS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn decimal_to_f64(d: Decimal) -> f64 {
    use std::str::FromStr;
    f64::from_str(&d.to_string()).unwrap_or(0.0)
}

fn f64_to_decimal(f: f64) -> Decimal {
    use std::str::FromStr;
    Decimal::from_str(&format!("{:.8}", f)).unwrap_or(Decimal::ZERO)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PriceLevel;
    use std::str::FromStr;
    
    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }
    
    #[test]
    fn test_rolling_stats() {
        let mut stats = RollingStats::new(5);
        
        // Add values
        for i in 1..=5 {
            stats.add(i as f64, 5);
        }
        
        // Mean of 1,2,3,4,5 = 3
        assert!((stats.mean() - 3.0).abs() < 0.001);
        
        // Stddev of 1,2,3,4,5 ≈ 1.414
        assert!((stats.stddev() - 1.414).abs() < 0.1);
    }
    
    #[test]
    fn test_whale_detection() {
        let config = WhaleConfig {
            window_size: 20,
            outlier_threshold: 2.0,
            min_absolute_size: dec("0.1"),
            analyze_depth: 10,
        };
        let detector = WhaleDetector::with_config(config);
        
        let mut book = OrderBook::new("BTC/USD");
        
        // Add normal sized orders
        for i in 1..=10 {
            let price = dec("50000") + Decimal::from(i * 10);
            book.bids.insert(price, PriceLevel {
                price,
                volume: dec("1.0"), // Normal size
                timestamp: Utc::now(),
            });
        }
        
        // First analysis builds up statistics
        let _ = detector.analyze(&book);
        
        // Add a whale order
        book.bids.insert(dec("50050"), PriceLevel {
            price: dec("50050"),
            volume: dec("50.0"), // 50x normal = whale!
            timestamp: Utc::now(),
        });
        
        let whales = detector.analyze(&book);
        
        // Should detect the whale
        assert!(!whales.is_empty(), "Should detect the whale order");
        assert!(whales[0].volume == dec("50.0"), "Should identify the 50 BTC order as whale");
    }
    
    #[test]
    fn test_no_false_positives() {
        let detector = WhaleDetector::new();
        
        let mut book = OrderBook::new("ETH/USD");
        
        // Add uniformly sized orders
        for i in 1..=20 {
            let price = dec("3000") + Decimal::from(i);
            book.bids.insert(price, PriceLevel {
                price,
                volume: dec("10.0"), // All same size
                timestamp: Utc::now(),
            });
        }
        
        let whales = detector.analyze(&book);
        
        // No whales when all orders are same size
        assert!(whales.is_empty(), "Should not detect whales when all orders are uniform");
    }
}
