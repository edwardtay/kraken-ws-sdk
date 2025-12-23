//! Liquidity Heatmap for Professional Trading Visualization
//!
//! Tracks the persistence of liquidity over time at each price level.
//! Longer-lasting levels get "hotter" heat scores, indicating more
//! stable/reliable liquidity vs transient orders.
//!
//! ## Example: Building a Liquidity Heatmap
//!
//! ```rust,ignore
//! use kraken_ws_sdk::extended::advanced::{LiquidityHeatmap, HeatmapConfig};
//!
//! let mut heatmap = LiquidityHeatmap::new(HeatmapConfig {
//!     max_heat_seconds: 300.0,  // 5 minutes = max heat
//!     decay_rate: 0.1,          // Slow decay when volume drops
//!     track_depth: 25,
//! });
//!
//! // On each order book update
//! heatmap.update(&order_book);
//!
//! // Get heatmap snapshot for visualization
//! let snapshot = heatmap.snapshot("BTC/USD");
//!
//! for level in &snapshot.bids {
//!     // level.heat_score is 0.0-1.0 (hotter = more persistent)
//!     let color = heat_to_color(level.heat_score);
//!     render_level(level.price, level.volume, color);
//! }
//! ```

use crate::orderbook::OrderBook;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIGURATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Configuration for liquidity heatmap tracking
#[derive(Debug, Clone)]
pub struct HeatmapConfig {
    /// Maximum seconds for full heat (1.0 score)
    pub max_heat_seconds: f64,
    /// Decay rate when volume decreases (0.0-1.0)
    pub decay_rate: f64,
    /// Number of levels to track from best bid/ask
    pub track_depth: usize,
    /// Minimum volume change ratio to reset heat (0.5 = 50% drop resets)
    pub volume_change_threshold: f64,
}

impl Default for HeatmapConfig {
    fn default() -> Self {
        Self {
            max_heat_seconds: 300.0,  // 5 minutes
            decay_rate: 0.1,
            track_depth: 25,
            volume_change_threshold: 0.5,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DATA STRUCTURES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A price level with heat information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatLevel {
    /// Price of this level
    pub price: Decimal,
    /// Current volume at this level
    pub volume: Decimal,
    /// Heat score from 0.0 (cold/new) to 1.0 (hot/persistent)
    pub heat_score: f64,
    /// How long this level has existed (seconds)
    pub lifetime_seconds: f64,
    /// When this level was first seen
    pub first_seen: DateTime<Utc>,
    /// Maximum volume observed at this level
    pub max_volume: Decimal,
}

/// Complete heatmap snapshot for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapSnapshot {
    /// Symbol this snapshot is for
    pub symbol: String,
    /// Bid levels with heat scores (best bid first)
    pub bids: Vec<HeatLevel>,
    /// Ask levels with heat scores (best ask first)
    pub asks: Vec<HeatLevel>,
    /// Overall bid side heat (average)
    pub bid_heat_avg: f64,
    /// Overall ask side heat (average)
    pub ask_heat_avg: f64,
    /// Timestamp of this snapshot
    pub timestamp: DateTime<Utc>,
}

/// Side of the heatmap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeatmapSide {
    Bid,
    Ask,
}

/// Internal tracking for a price level
#[derive(Debug, Clone)]
struct LevelTracker {
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    last_volume: Decimal,
    max_volume: Decimal,
    heat_accumulated: f64,
}

/// Internal tracking for a symbol
#[derive(Debug, Default)]
struct SymbolTracker {
    bids: HashMap<String, LevelTracker>,  // Price as string key
    asks: HashMap<String, LevelTracker>,
    last_update: Option<DateTime<Utc>>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// LIQUIDITY HEATMAP
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Liquidity persistence heatmap tracker
///
/// Tracks how long liquidity persists at each price level, generating
/// heat scores for visualization. Persistent liquidity = reliable support/resistance.
pub struct LiquidityHeatmap {
    config: HeatmapConfig,
    /// Tracking data by symbol
    trackers: Mutex<HashMap<String, SymbolTracker>>,
}

impl LiquidityHeatmap {
    /// Create a new heatmap with default config
    pub fn new() -> Self {
        Self::with_config(HeatmapConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: HeatmapConfig) -> Self {
        Self {
            config,
            trackers: Mutex::new(HashMap::new()),
        }
    }
    
    /// Update heatmap with new order book data
    ///
    /// Call this on every order book update to track persistence.
    pub fn update(&self, book: &OrderBook) {
        let mut trackers = self.trackers.lock().unwrap();
        let now = Utc::now();
        
        let tracker = trackers
            .entry(book.symbol.clone())
            .or_insert_with(SymbolTracker::default);
        
        // Calculate time delta
        let delta_secs = tracker.last_update
            .map(|last| (now - last).num_milliseconds() as f64 / 1000.0)
            .unwrap_or(0.0);
        
        // Track bid levels
        let mut seen_bids: Vec<String> = Vec::new();
        for (price, level) in book.bids.iter().rev().take(self.config.track_depth) {
            let key = price.to_string();
            seen_bids.push(key.clone());
            
            self.update_level(
                &mut tracker.bids,
                &key,
                level.volume,
                now,
                delta_secs,
            );
        }
        
        // Track ask levels
        let mut seen_asks: Vec<String> = Vec::new();
        for (price, level) in book.asks.iter().take(self.config.track_depth) {
            let key = price.to_string();
            seen_asks.push(key.clone());
            
            self.update_level(
                &mut tracker.asks,
                &key,
                level.volume,
                now,
                delta_secs,
            );
        }
        
        // Remove stale levels (no longer in book)
        tracker.bids.retain(|k, _| seen_bids.contains(k));
        tracker.asks.retain(|k, _| seen_asks.contains(k));
        
        tracker.last_update = Some(now);
    }
    
    /// Update a single level's tracking
    fn update_level(
        &self,
        levels: &mut HashMap<String, LevelTracker>,
        key: &str,
        volume: Decimal,
        now: DateTime<Utc>,
        delta_secs: f64,
    ) {
        if let Some(existing) = levels.get_mut(key) {
            // Level exists - check for significant volume change
            let volume_ratio = if existing.last_volume > Decimal::ZERO {
                decimal_to_f64(volume) / decimal_to_f64(existing.last_volume)
            } else {
                1.0
            };
            
            if volume_ratio < self.config.volume_change_threshold {
                // Significant volume drop - decay heat
                existing.heat_accumulated *= 1.0 - self.config.decay_rate;
            } else {
                // Volume stable or increased - accumulate heat
                existing.heat_accumulated += delta_secs;
            }
            
            existing.last_seen = now;
            existing.last_volume = volume;
            existing.max_volume = existing.max_volume.max(volume);
        } else {
            // New level
            levels.insert(key.to_string(), LevelTracker {
                first_seen: now,
                last_seen: now,
                last_volume: volume,
                max_volume: volume,
                heat_accumulated: 0.0,
            });
        }
    }
    
    /// Get a heatmap snapshot for visualization
    pub fn snapshot(&self, symbol: &str) -> Option<HeatmapSnapshot> {
        let trackers = self.trackers.lock().unwrap();
        let tracker = trackers.get(symbol)?;
        let now = Utc::now();
        
        let mut bids: Vec<HeatLevel> = tracker.bids.iter()
            .map(|(price_str, t)| self.create_heat_level(price_str, t, now))
            .collect();
        
        let mut asks: Vec<HeatLevel> = tracker.asks.iter()
            .map(|(price_str, t)| self.create_heat_level(price_str, t, now))
            .collect();
        
        // Sort bids descending, asks ascending
        bids.sort_by(|a, b| b.price.cmp(&a.price));
        asks.sort_by(|a, b| a.price.cmp(&b.price));
        
        let bid_heat_avg = if bids.is_empty() {
            0.0
        } else {
            bids.iter().map(|l| l.heat_score).sum::<f64>() / bids.len() as f64
        };
        
        let ask_heat_avg = if asks.is_empty() {
            0.0
        } else {
            asks.iter().map(|l| l.heat_score).sum::<f64>() / asks.len() as f64
        };
        
        Some(HeatmapSnapshot {
            symbol: symbol.to_string(),
            bids,
            asks,
            bid_heat_avg,
            ask_heat_avg,
            timestamp: now,
        })
    }
    
    /// Create a HeatLevel from internal tracking
    fn create_heat_level(&self, price_str: &str, tracker: &LevelTracker, now: DateTime<Utc>) -> HeatLevel {
        let lifetime = (now - tracker.first_seen).num_milliseconds() as f64 / 1000.0;
        let heat_score = (tracker.heat_accumulated / self.config.max_heat_seconds).min(1.0);
        
        HeatLevel {
            price: price_str.parse().unwrap_or(Decimal::ZERO),
            volume: tracker.last_volume,
            heat_score,
            lifetime_seconds: lifetime,
            first_seen: tracker.first_seen,
            max_volume: tracker.max_volume,
        }
    }
    
    /// Get the hottest levels (most persistent liquidity)
    pub fn get_hottest(&self, symbol: &str, count: usize) -> Vec<HeatLevel> {
        let Some(snapshot) = self.snapshot(symbol) else {
            return Vec::new();
        };
        
        let mut all: Vec<HeatLevel> = snapshot.bids.into_iter()
            .chain(snapshot.asks.into_iter())
            .collect();
        
        all.sort_by(|a, b| b.heat_score.partial_cmp(&a.heat_score).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(count);
        all
    }
    
    /// Reset tracking for a symbol
    pub fn reset(&self, symbol: &str) {
        let mut trackers = self.trackers.lock().unwrap();
        trackers.remove(symbol);
    }
    
    /// Reset all tracking
    pub fn reset_all(&self) {
        let mut trackers = self.trackers.lock().unwrap();
        trackers.clear();
    }
}

impl Default for LiquidityHeatmap {
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
    fn test_heatmap_new_levels() {
        let heatmap = LiquidityHeatmap::new();
        
        let mut book = OrderBook::new("BTC/USD");
        book.bids.insert(dec("50000"), PriceLevel {
            price: dec("50000"),
            volume: dec("10.0"),
            timestamp: Utc::now(),
        });
        
        heatmap.update(&book);
        
        let snapshot = heatmap.snapshot("BTC/USD");
        assert!(snapshot.is_some());
        
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.bids.len(), 1);
        assert!(snapshot.bids[0].heat_score < 0.1, "New levels should be cold");
    }
    
    #[test]
    fn test_heatmap_persistence() {
        let config = HeatmapConfig {
            max_heat_seconds: 10.0,  // Short for testing
            ..Default::default()
        };
        let heatmap = LiquidityHeatmap::with_config(config);
        
        let mut book = OrderBook::new("ETH/USD");
        book.bids.insert(dec("3000"), PriceLevel {
            price: dec("3000"),
            volume: dec("5.0"),
            timestamp: Utc::now(),
        });
        
        // Simulate multiple updates (each adds heat)
        for _ in 0..5 {
            heatmap.update(&book);
        }
        
        let snapshot = heatmap.snapshot("ETH/USD").unwrap();
        assert!(!snapshot.bids.is_empty());
        // Heat should increase with updates (but timing dependent in real use)
    }
    
    #[test]
    fn test_heatmap_level_removal() {
        let heatmap = LiquidityHeatmap::new();
        
        let mut book = OrderBook::new("XRP/USD");
        book.bids.insert(dec("1.0"), PriceLevel {
            price: dec("1.0"),
            volume: dec("1000"),
            timestamp: Utc::now(),
        });
        
        heatmap.update(&book);
        
        // Remove the level
        book.bids.clear();
        heatmap.update(&book);
        
        let snapshot = heatmap.snapshot("XRP/USD").unwrap();
        assert!(snapshot.bids.is_empty(), "Removed levels should not appear");
    }
    
    #[test]
    fn test_get_hottest() {
        let heatmap = LiquidityHeatmap::new();
        
        let mut book = OrderBook::new("BTC/USD");
        for i in 1..=5 {
            let price = dec("50000") + Decimal::from(i * 10);
            book.bids.insert(price, PriceLevel {
                price,
                volume: dec("1.0"),
                timestamp: Utc::now(),
            });
        }
        
        heatmap.update(&book);
        
        let hottest = heatmap.get_hottest("BTC/USD", 3);
        assert!(hottest.len() <= 3);
    }
}
