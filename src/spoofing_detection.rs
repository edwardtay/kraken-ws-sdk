//! Spoofing Detection for Professional Trading Visualization
//!
//! Detects potential spoofing patterns: large orders that appear and
//! quickly vanish without executing. These patterns often indicate
//! market manipulation attempts.
//!
//! ## Example: Detecting Spoofing
//!
//! ```rust,ignore
//! use kraken_ws_sdk::extended::advanced::{SpoofingDetector, SpoofingConfig};
//! use kraken_ws_sdk::orderflow::{OrderFlowTracker, FlowEvent};
//!
//! let detector = SpoofingDetector::new(SpoofingConfig {
//!     min_size_threshold: dec!(5.0),  // Only track large orders
//!     max_lifetime_ms: 5000,          // Suspicious if < 5 seconds
//!     require_no_trades: true,        // Must not have traded
//! });
//!
//! // On each order flow event
//! for event in flow_events {
//!     if let Some(alert) = detector.process_event(&event) {
//!         println!("⚠️ SPOOFING DETECTED @ {}: {} vanished in {}ms",
//!             alert.price, alert.volume, alert.lifetime_ms);
//!     }
//! }
//! ```

use crate::orderflow::{FlowEvent, FlowEventType, FlowSide};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CONFIGURATION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Configuration for spoofing detection
#[derive(Debug, Clone)]
pub struct SpoofingConfig {
    /// Minimum order size to track (filters small orders)
    pub min_size_threshold: Decimal,
    /// Maximum lifetime in milliseconds to consider suspicious
    pub max_lifetime_ms: u64,
    /// Whether to require no trades at the level during lifetime
    pub require_no_trades: bool,
    /// Maximum pending appearances to track per symbol
    pub max_pending_per_symbol: usize,
    /// Expiry time for pending appearances (ms) - if no disappearance, forget it
    pub pending_expiry_ms: u64,
}

impl Default for SpoofingConfig {
    fn default() -> Self {
        Self {
            min_size_threshold: Decimal::from(5),
            max_lifetime_ms: 5000,   // 5 seconds
            require_no_trades: true,
            max_pending_per_symbol: 100,
            pending_expiry_ms: 60000, // 1 minute
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// DATA STRUCTURES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A detected spoofing alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpoofingAlert {
    /// Symbol where spoofing was detected
    pub symbol: String,
    /// Price level of the spoofed order
    pub price: Decimal,
    /// Volume of the spoofed order
    pub volume: Decimal,
    /// Side of the order book
    pub side: SpoofSide,
    /// When the order appeared
    pub appeared_at: DateTime<Utc>,
    /// When the order disappeared
    pub disappeared_at: DateTime<Utc>,
    /// How long the order existed (ms)
    pub lifetime_ms: u64,
    /// Suspicion score from 0.0 to 1.0 (higher = more suspicious)
    pub suspicion_score: f64,
    /// Whether any trades occurred at this level during lifetime
    pub trades_occurred: bool,
}

/// Side of the spoofed order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpoofSide {
    Bid,
    Ask,
}

impl From<FlowSide> for SpoofSide {
    fn from(side: FlowSide) -> Self {
        match side {
            FlowSide::Bid => SpoofSide::Bid,
            FlowSide::Ask => SpoofSide::Ask,
        }
    }
}

/// Pending appearance waiting for potential disappearance
#[derive(Debug, Clone)]
struct PendingAppearance {
    symbol: String,
    price: Decimal,
    volume: Decimal,
    side: SpoofSide,
    appeared_at: DateTime<Utc>,
    sequence: u64,
}

/// Key for tracking pending appearances
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct AppearanceKey {
    symbol: String,
    price: String,  // Decimal as string for HashMap key
    side: SpoofSide,
}

/// Trade tracking for a price level
#[derive(Debug, Clone, Default)]
struct TradeTracker {
    last_trade_at: Option<DateTime<Utc>>,
    trade_count: u64,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SPOOFING DETECTOR
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Spoofing pattern detector
///
/// Correlates LargeOrderAppeared and LargeOrderDisappeared events to detect
/// orders that vanish suspiciously fast without trading.
pub struct SpoofingDetector {
    config: SpoofingConfig,
    /// Pending large order appearances
    pending: Mutex<HashMap<AppearanceKey, PendingAppearance>>,
    /// Trade tracking per level
    trades: Mutex<HashMap<AppearanceKey, TradeTracker>>,
    /// Alert history
    alerts: Mutex<Vec<SpoofingAlert>>,
}

impl SpoofingDetector {
    /// Create a new spoofing detector with default config
    pub fn new() -> Self {
        Self::with_config(SpoofingConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: SpoofingConfig) -> Self {
        Self {
            config,
            pending: Mutex::new(HashMap::new()),
            trades: Mutex::new(HashMap::new()),
            alerts: Mutex::new(Vec::new()),
        }
    }
    
    /// Process a flow event and potentially return a spoofing alert
    ///
    /// Call this for every FlowEvent from OrderFlowTracker.
    pub fn process_event(&self, event: &FlowEvent) -> Option<SpoofingAlert> {
        // First, clean up expired pending appearances
        self.cleanup_expired();
        
        match &event.event_type {
            FlowEventType::LargeOrderAppeared => {
                self.handle_appearance(event);
                None
            }
            FlowEventType::LargeOrderDisappeared => {
                self.handle_disappearance(event)
            }
            _ => None,
        }
    }
    
    /// Process multiple events at once
    pub fn process_events(&self, events: &[FlowEvent]) -> Vec<SpoofingAlert> {
        events.iter()
            .filter_map(|e| self.process_event(e))
            .collect()
    }
    
    /// Handle a large order appearance
    fn handle_appearance(&self, event: &FlowEvent) {
        // Only track if above threshold
        if event.current_volume < self.config.min_size_threshold {
            return;
        }
        
        let key = AppearanceKey {
            symbol: event.symbol.clone(),
            price: event.price.to_string(),
            side: event.side.into(),
        };
        
        let appearance = PendingAppearance {
            symbol: event.symbol.clone(),
            price: event.price,
            volume: event.current_volume,
            side: event.side.into(),
            appeared_at: event.timestamp,
            sequence: event.sequence,
        };
        
        let mut pending = self.pending.lock().unwrap();
        
        // Limit pending per symbol
        let symbol_count = pending.values()
            .filter(|p| p.symbol == event.symbol)
            .count();
        
        if symbol_count < self.config.max_pending_per_symbol {
            pending.insert(key, appearance);
        }
    }
    
    /// Handle a large order disappearance - check for spoofing
    fn handle_disappearance(&self, event: &FlowEvent) -> Option<SpoofingAlert> {
        let key = AppearanceKey {
            symbol: event.symbol.clone(),
            price: event.price.to_string(),
            side: event.side.into(),
        };
        
        let mut pending = self.pending.lock().unwrap();
        let appearance = pending.remove(&key)?;
        
        // Calculate lifetime
        let lifetime_ms = (event.timestamp - appearance.appeared_at)
            .num_milliseconds() as u64;
        
        // Check if suspicious (fast disappearance)
        if lifetime_ms > self.config.max_lifetime_ms {
            return None;  // Too long to be suspicious
        }
        
        // Check for trades at this level (if required)
        let trades_occurred = if self.config.require_no_trades {
            let trades = self.trades.lock().unwrap();
            trades.get(&key)
                .map(|t| t.last_trade_at
                    .map(|tt| tt >= appearance.appeared_at)
                    .unwrap_or(false))
                .unwrap_or(false)
        } else {
            false
        };
        
        if self.config.require_no_trades && trades_occurred {
            return None;  // Had trades, probably legitimate
        }
        
        // Calculate suspicion score
        // Score is higher for:
        // - Shorter lifetime (faster = more suspicious)
        // - Larger size (bigger = more impact)
        // - No trades (more suspicious)
        let time_factor = 1.0 - (lifetime_ms as f64 / self.config.max_lifetime_ms as f64);
        let size_factor = (decimal_to_f64(appearance.volume) / decimal_to_f64(self.config.min_size_threshold)).min(2.0) / 2.0;
        let trade_factor = if trades_occurred { 0.5 } else { 1.0 };
        
        let suspicion_score = (time_factor * 0.5 + size_factor * 0.3 + trade_factor * 0.2).min(1.0);
        
        let alert = SpoofingAlert {
            symbol: event.symbol.clone(),
            price: event.price,
            volume: appearance.volume,
            side: appearance.side,
            appeared_at: appearance.appeared_at,
            disappeared_at: event.timestamp,
            lifetime_ms,
            suspicion_score,
            trades_occurred,
        };
        
        // Store in history
        let mut alerts = self.alerts.lock().unwrap();
        alerts.push(alert.clone());
        
        // Limit history size
        while alerts.len() > 1000 {
            alerts.remove(0);
        }
        
        Some(alert)
    }
    
    /// Record a trade at a price level (for trade-based filtering)
    pub fn record_trade(&self, symbol: &str, price: Decimal, side: SpoofSide) {
        let key = AppearanceKey {
            symbol: symbol.to_string(),
            price: price.to_string(),
            side,
        };
        
        let mut trades = self.trades.lock().unwrap();
        let tracker = trades.entry(key).or_default();
        tracker.last_trade_at = Some(Utc::now());
        tracker.trade_count += 1;
    }
    
    /// Clean up expired pending appearances
    fn cleanup_expired(&self) {
        let now = Utc::now();
        let expiry_ms = self.config.pending_expiry_ms as i64;
        
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, v| {
            (now - v.appeared_at).num_milliseconds() < expiry_ms
        });
        
        // Also clean up old trade tracking
        let mut trades = self.trades.lock().unwrap();
        trades.retain(|_, v| {
            v.last_trade_at
                .map(|t| (now - t).num_milliseconds() < expiry_ms)
                .unwrap_or(true)
        });
    }
    
    /// Get recent alerts
    pub fn get_recent_alerts(&self, count: usize) -> Vec<SpoofingAlert> {
        let alerts = self.alerts.lock().unwrap();
        alerts.iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }
    
    /// Get alerts for a specific symbol
    pub fn get_alerts_for_symbol(&self, symbol: &str) -> Vec<SpoofingAlert> {
        let alerts = self.alerts.lock().unwrap();
        alerts.iter()
            .filter(|a| a.symbol == symbol)
            .cloned()
            .collect()
    }
    
    /// Get spoofing statistics for a symbol
    pub fn get_stats(&self, symbol: &str) -> SpoofingStats {
        let alerts = self.alerts.lock().unwrap();
        let symbol_alerts: Vec<_> = alerts.iter()
            .filter(|a| a.symbol == symbol)
            .collect();
        
        if symbol_alerts.is_empty() {
            return SpoofingStats::default();
        }
        
        let total = symbol_alerts.len();
        let avg_lifetime = symbol_alerts.iter()
            .map(|a| a.lifetime_ms as f64)
            .sum::<f64>() / total as f64;
        let avg_suspicion = symbol_alerts.iter()
            .map(|a| a.suspicion_score)
            .sum::<f64>() / total as f64;
        let bid_count = symbol_alerts.iter()
            .filter(|a| a.side == SpoofSide::Bid)
            .count();
        let ask_count = total - bid_count;
        
        SpoofingStats {
            total_alerts: total,
            avg_lifetime_ms: avg_lifetime,
            avg_suspicion_score: avg_suspicion,
            bid_spoofs: bid_count,
            ask_spoofs: ask_count,
        }
    }
    
    /// Reset all state
    pub fn reset(&self) {
        let mut pending = self.pending.lock().unwrap();
        let mut trades = self.trades.lock().unwrap();
        let mut alerts = self.alerts.lock().unwrap();
        
        pending.clear();
        trades.clear();
        alerts.clear();
    }
}

impl Default for SpoofingDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about spoofing detections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpoofingStats {
    pub total_alerts: usize,
    pub avg_lifetime_ms: f64,
    pub avg_suspicion_score: f64,
    pub bid_spoofs: usize,
    pub ask_spoofs: usize,
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
    use std::str::FromStr;
    
    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }
    
    fn make_appear_event(symbol: &str, price: Decimal, volume: Decimal, seq: u64) -> FlowEvent {
        FlowEvent {
            symbol: symbol.to_string(),
            price,
            side: FlowSide::Bid,
            event_type: FlowEventType::LargeOrderAppeared,
            current_volume: volume,
            previous_volume: Decimal::ZERO,
            timestamp: Utc::now(),
            sequence: seq,
        }
    }
    
    fn make_disappear_event(symbol: &str, price: Decimal, volume: Decimal, seq: u64) -> FlowEvent {
        FlowEvent {
            symbol: symbol.to_string(),
            price,
            side: FlowSide::Bid,
            event_type: FlowEventType::LargeOrderDisappeared,
            current_volume: Decimal::ZERO,
            previous_volume: volume,
            timestamp: Utc::now(),
            sequence: seq,
        }
    }
    
    #[test]
    fn test_spoofing_detection() {
        let config = SpoofingConfig {
            min_size_threshold: dec("1.0"),
            max_lifetime_ms: 10000,  // 10 seconds
            require_no_trades: false,  // Don't require trade check for this test
            ..Default::default()
        };
        let detector = SpoofingDetector::with_config(config);
        
        // Large order appears
        let appear = make_appear_event("BTC/USD", dec("50000"), dec("10.0"), 1);
        let result = detector.process_event(&appear);
        assert!(result.is_none(), "Appearance should not trigger alert");
        
        // Same order disappears quickly
        let disappear = make_disappear_event("BTC/USD", dec("50000"), dec("10.0"), 2);
        let result = detector.process_event(&disappear);
        
        assert!(result.is_some(), "Fast disappearance should trigger alert");
        let alert = result.unwrap();
        assert_eq!(alert.symbol, "BTC/USD");
        assert_eq!(alert.volume, dec("10.0"));
        assert!(alert.suspicion_score > 0.0);
    }
    
    #[test]
    fn test_no_alert_for_small_orders() {
        let config = SpoofingConfig {
            min_size_threshold: dec("10.0"),  // Large threshold
            ..Default::default()
        };
        let detector = SpoofingDetector::with_config(config);
        
        // Small order appears
        let appear = make_appear_event("BTC/USD", dec("50000"), dec("1.0"), 1);
        detector.process_event(&appear);
        
        // Same order disappears
        let disappear = make_disappear_event("BTC/USD", dec("50000"), dec("1.0"), 2);
        let result = detector.process_event(&disappear);
        
        assert!(result.is_none(), "Small orders should not trigger alerts");
    }
    
    #[test]
    fn test_get_recent_alerts() {
        let config = SpoofingConfig {
            min_size_threshold: dec("1.0"),
            require_no_trades: false,
            ..Default::default()
        };
        let detector = SpoofingDetector::with_config(config);
        
        // Create multiple alerts
        for i in 1..=5 {
            let price = dec("50000") + Decimal::from(i * 10);
            let appear = make_appear_event("BTC/USD", price, dec("10.0"), i * 2 - 1);
            detector.process_event(&appear);
            let disappear = make_disappear_event("BTC/USD", price, dec("10.0"), i * 2);
            detector.process_event(&disappear);
        }
        
        let recent = detector.get_recent_alerts(3);
        assert_eq!(recent.len(), 3);
    }
    
    #[test]
    fn test_stats() {
        let config = SpoofingConfig {
            min_size_threshold: dec("1.0"),
            require_no_trades: false,
            ..Default::default()
        };
        let detector = SpoofingDetector::with_config(config);
        
        // Create an alert
        let appear = make_appear_event("ETH/USD", dec("3000"), dec("5.0"), 1);
        detector.process_event(&appear);
        let disappear = make_disappear_event("ETH/USD", dec("3000"), dec("5.0"), 2);
        detector.process_event(&disappear);
        
        let stats = detector.get_stats("ETH/USD");
        assert_eq!(stats.total_alerts, 1);
        assert_eq!(stats.bid_spoofs, 1);
    }
}
