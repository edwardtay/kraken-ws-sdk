//! Order Flow Tracking for Professional Trading Visualization
//!
//! Detects and tracks changes in order book liquidity:
//! - Large order appearance/disappearance (order flow highlighting)
//! - Size changes at price levels (delta tracking)
//! - Recent trades aligned with price levels
//!
//! ## Example: Order Flow Visualization
//!
//! ```rust,ignore
//! use kraken_ws_sdk::orderflow::{OrderFlowTracker, FlowEvent, FlowEventType};
//!
//! let mut tracker = OrderFlowTracker::new(OrderFlowConfig {
//!     large_order_threshold: dec!(10.0),  // 10 BTC = large order
//!     track_depth: 20,
//!     ..Default::default()
//! });
//!
//! // On each order book update
//! let events = tracker.track_update(&symbol, &new_book);
//!
//! for event in events {
//!     match event.event_type {
//!         FlowEventType::LargeOrderAppeared => {
//!             // Flash green animation at this price level
//!             animate_level(event.price, "appear");
//!         }
//!         FlowEventType::LargeOrderDisappeared => {
//!             // Flash red animation
//!             animate_level(event.price, "disappear");
//!         }
//!         FlowEventType::SizeIncreased { delta } => {
//!             // Show size increase indicator
//!         }
//!         _ => {}
//!     }
//! }
//! ```

use crate::data::{PriceLevel, TradeData, TradeSide};
use crate::orderbook::OrderBook;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ORDER FLOW TRACKER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Configuration for order flow tracking
#[derive(Debug, Clone)]
pub struct OrderFlowConfig {
    /// Minimum volume to consider as a "large" order
    pub large_order_threshold: Decimal,
    /// Minimum volume change to emit a size change event
    pub min_size_change: Decimal,
    /// Number of levels to track from best bid/ask
    pub track_depth: usize,
    /// Maximum number of events to keep in history
    pub max_history: usize,
    /// Whether to track size changes (can be noisy)
    pub track_size_changes: bool,
}

impl Default for OrderFlowConfig {
    fn default() -> Self {
        Self {
            large_order_threshold: Decimal::from(10),
            min_size_change: Decimal::from(1),
            track_depth: 20,
            max_history: 1000,
            track_size_changes: true,
        }
    }
}

/// Order flow event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowEventType {
    /// A large order appeared at this price level
    LargeOrderAppeared,
    /// A large order disappeared from this price level
    LargeOrderDisappeared,
    /// Size increased at this level
    SizeIncreased { delta: Decimal },
    /// Size decreased at this level
    SizeDecreased { delta: Decimal },
    /// New price level added
    LevelAdded,
    /// Price level removed
    LevelRemoved,
    /// Best bid changed
    BestBidChanged { old: Decimal, new: Decimal },
    /// Best ask changed
    BestAskChanged { old: Decimal, new: Decimal },
}

/// A single order flow event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEvent {
    /// Symbol this event is for
    pub symbol: String,
    /// Price level where event occurred
    pub price: Decimal,
    /// Side (bid or ask)
    pub side: FlowSide,
    /// Type of event
    pub event_type: FlowEventType,
    /// Current volume at this level (after change)
    pub current_volume: Decimal,
    /// Previous volume at this level (before change)
    pub previous_volume: Decimal,
    /// Timestamp of event
    pub timestamp: DateTime<Utc>,
    /// Sequence number for ordering
    pub sequence: u64,
}

/// Side of the order book
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSide {
    Bid,
    Ask,
}

/// Snapshot of a price level for comparison
#[derive(Debug, Clone)]
struct LevelSnapshot {
    price: Decimal,
    volume: Decimal,
    timestamp: DateTime<Utc>,
}

/// Production-grade order flow tracker
pub struct OrderFlowTracker {
    config: OrderFlowConfig,
    /// Previous snapshots by symbol
    previous_books: Mutex<HashMap<String, BookSnapshot>>,
    /// Event history
    event_history: Mutex<VecDeque<FlowEvent>>,
    /// Event sequence counter
    sequence: Mutex<u64>,
    /// Callbacks for flow events
    callbacks: Mutex<Vec<Arc<dyn Fn(FlowEvent) + Send + Sync>>>,
}

/// Snapshot of an order book for comparison
#[derive(Debug, Clone)]
struct BookSnapshot {
    symbol: String,
    bids: BTreeMap<Decimal, LevelSnapshot>,
    asks: BTreeMap<Decimal, LevelSnapshot>,
    best_bid: Option<Decimal>,
    best_ask: Option<Decimal>,
    timestamp: DateTime<Utc>,
}

impl OrderFlowTracker {
    /// Create a new order flow tracker with default config
    pub fn new() -> Self {
        Self::with_config(OrderFlowConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: OrderFlowConfig) -> Self {
        Self {
            config,
            previous_books: Mutex::new(HashMap::new()),
            event_history: Mutex::new(VecDeque::new()),
            sequence: Mutex::new(0),
            callbacks: Mutex::new(Vec::new()),
        }
    }
    
    /// Register callback for flow events
    pub fn on_event<F>(&self, callback: F)
    where
        F: Fn(FlowEvent) + Send + Sync + 'static,
    {
        self.callbacks.lock().unwrap().push(Arc::new(callback));
    }
    
    /// Track an order book update and detect flow events
    ///
    /// Returns a list of events detected in this update.
    pub fn track_update(&self, book: &OrderBook) -> Vec<FlowEvent> {
        let mut events = Vec::new();
        let mut previous_books = self.previous_books.lock().unwrap();
        
        let current_snapshot = self.create_snapshot(book);
        
        if let Some(prev_snapshot) = previous_books.get(&book.symbol) {
            // Detect events by comparing snapshots
            events.extend(self.detect_bid_events(prev_snapshot, &current_snapshot));
            events.extend(self.detect_ask_events(prev_snapshot, &current_snapshot));
            events.extend(self.detect_bbo_changes(prev_snapshot, &current_snapshot));
        }
        
        // Store current as previous for next comparison
        previous_books.insert(book.symbol.clone(), current_snapshot);
        
        // Record events and emit callbacks
        for event in &events {
            self.record_event(event.clone());
        }
        
        events
    }
    
    /// Create a snapshot from an order book
    fn create_snapshot(&self, book: &OrderBook) -> BookSnapshot {
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();
        
        // Snapshot top N bid levels
        for (price, level) in book.bids.iter().rev().take(self.config.track_depth) {
            bids.insert(*price, LevelSnapshot {
                price: *price,
                volume: level.volume,
                timestamp: level.timestamp,
            });
        }
        
        // Snapshot top N ask levels
        for (price, level) in book.asks.iter().take(self.config.track_depth) {
            asks.insert(*price, LevelSnapshot {
                price: *price,
                volume: level.volume,
                timestamp: level.timestamp,
            });
        }
        
        BookSnapshot {
            symbol: book.symbol.clone(),
            bids,
            asks,
            best_bid: book.bids.keys().next_back().copied(),
            best_ask: book.asks.keys().next().copied(),
            timestamp: book.last_update,
        }
    }
    
    /// Detect events on the bid side
    fn detect_bid_events(&self, prev: &BookSnapshot, curr: &BookSnapshot) -> Vec<FlowEvent> {
        self.detect_side_events(
            &prev.bids,
            &curr.bids,
            &curr.symbol,
            FlowSide::Bid,
        )
    }
    
    /// Detect events on the ask side
    fn detect_ask_events(&self, prev: &BookSnapshot, curr: &BookSnapshot) -> Vec<FlowEvent> {
        self.detect_side_events(
            &prev.asks,
            &curr.asks,
            &curr.symbol,
            FlowSide::Ask,
        )
    }
    
    /// Detect events on a single side
    fn detect_side_events(
        &self,
        prev_levels: &BTreeMap<Decimal, LevelSnapshot>,
        curr_levels: &BTreeMap<Decimal, LevelSnapshot>,
        symbol: &str,
        side: FlowSide,
    ) -> Vec<FlowEvent> {
        let mut events = Vec::new();
        let threshold = self.config.large_order_threshold;
        let min_change = self.config.min_size_change;
        
        // Check for new levels and size changes
        for (price, curr_level) in curr_levels {
            if let Some(prev_level) = prev_levels.get(price) {
                // Level existed before - check for size changes
                let delta = curr_level.volume - prev_level.volume;
                
                if delta.abs() >= min_change && self.config.track_size_changes {
                    let event_type = if delta > Decimal::ZERO {
                        // Check if this is a large order appearing
                        if curr_level.volume >= threshold && prev_level.volume < threshold {
                            FlowEventType::LargeOrderAppeared
                        } else {
                            FlowEventType::SizeIncreased { delta }
                        }
                    } else {
                        // Check if a large order disappeared
                        if prev_level.volume >= threshold && curr_level.volume < threshold {
                            FlowEventType::LargeOrderDisappeared
                        } else {
                            FlowEventType::SizeDecreased { delta: delta.abs() }
                        }
                    };
                    
                    events.push(self.create_event(
                        symbol,
                        *price,
                        side,
                        event_type,
                        curr_level.volume,
                        prev_level.volume,
                    ));
                }
            } else {
                // New level
                let event_type = if curr_level.volume >= threshold {
                    FlowEventType::LargeOrderAppeared
                } else {
                    FlowEventType::LevelAdded
                };
                
                events.push(self.create_event(
                    symbol,
                    *price,
                    side,
                    event_type,
                    curr_level.volume,
                    Decimal::ZERO,
                ));
            }
        }
        
        // Check for removed levels
        for (price, prev_level) in prev_levels {
            if !curr_levels.contains_key(price) {
                let event_type = if prev_level.volume >= threshold {
                    FlowEventType::LargeOrderDisappeared
                } else {
                    FlowEventType::LevelRemoved
                };
                
                events.push(self.create_event(
                    symbol,
                    *price,
                    side,
                    event_type,
                    Decimal::ZERO,
                    prev_level.volume,
                ));
            }
        }
        
        events
    }
    
    /// Detect best bid/offer changes
    fn detect_bbo_changes(&self, prev: &BookSnapshot, curr: &BookSnapshot) -> Vec<FlowEvent> {
        let mut events = Vec::new();
        
        // Best bid change
        if prev.best_bid != curr.best_bid {
            if let (Some(old), Some(new)) = (prev.best_bid, curr.best_bid) {
                events.push(self.create_event(
                    &curr.symbol,
                    new,
                    FlowSide::Bid,
                    FlowEventType::BestBidChanged { old, new },
                    Decimal::ZERO,
                    Decimal::ZERO,
                ));
            }
        }
        
        // Best ask change
        if prev.best_ask != curr.best_ask {
            if let (Some(old), Some(new)) = (prev.best_ask, curr.best_ask) {
                events.push(self.create_event(
                    &curr.symbol,
                    new,
                    FlowSide::Ask,
                    FlowEventType::BestAskChanged { old, new },
                    Decimal::ZERO,
                    Decimal::ZERO,
                ));
            }
        }
        
        events
    }
    
    /// Create a flow event
    fn create_event(
        &self,
        symbol: &str,
        price: Decimal,
        side: FlowSide,
        event_type: FlowEventType,
        current_volume: Decimal,
        previous_volume: Decimal,
    ) -> FlowEvent {
        let mut seq = self.sequence.lock().unwrap();
        *seq += 1;
        
        FlowEvent {
            symbol: symbol.to_string(),
            price,
            side,
            event_type,
            current_volume,
            previous_volume,
            timestamp: Utc::now(),
            sequence: *seq,
        }
    }
    
    /// Record an event and emit callbacks
    fn record_event(&self, event: FlowEvent) {
        // Add to history
        let mut history = self.event_history.lock().unwrap();
        history.push_back(event.clone());
        while history.len() > self.config.max_history {
            history.pop_front();
        }
        drop(history);
        
        // Emit callbacks
        let callbacks = self.callbacks.lock().unwrap();
        for callback in callbacks.iter() {
            callback(event.clone());
        }
    }
    
    /// Get recent flow events
    pub fn get_recent_events(&self, limit: usize) -> Vec<FlowEvent> {
        let history = self.event_history.lock().unwrap();
        history.iter().rev().take(limit).cloned().collect()
    }
    
    /// Get events for a specific symbol
    pub fn get_events_for_symbol(&self, symbol: &str, limit: usize) -> Vec<FlowEvent> {
        let history = self.event_history.lock().unwrap();
        history
            .iter()
            .rev()
            .filter(|e| e.symbol == symbol)
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get only large order events
    pub fn get_large_order_events(&self, limit: usize) -> Vec<FlowEvent> {
        let history = self.event_history.lock().unwrap();
        history
            .iter()
            .rev()
            .filter(|e| matches!(
                e.event_type,
                FlowEventType::LargeOrderAppeared | FlowEventType::LargeOrderDisappeared
            ))
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Clear history for a symbol
    pub fn clear_symbol(&self, symbol: &str) {
        self.previous_books.lock().unwrap().remove(symbol);
    }
    
    /// Clear all history
    pub fn clear_all(&self) {
        self.previous_books.lock().unwrap().clear();
        self.event_history.lock().unwrap().clear();
    }
}

impl Default for OrderFlowTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TRADES BY PRICE LEVEL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Configuration for trade tracking by price level
#[derive(Debug, Clone)]
pub struct TradeOverlayConfig {
    /// Maximum trades to keep per price level
    pub max_trades_per_level: usize,
    /// Maximum price levels to track
    pub max_levels: usize,
    /// Time window for trade aggregation (seconds)
    pub aggregation_window_secs: u64,
    /// Price rounding for level matching (tick size)
    pub price_precision: Option<Decimal>,
}

impl Default for TradeOverlayConfig {
    fn default() -> Self {
        Self {
            max_trades_per_level: 50,
            max_levels: 100,
            aggregation_window_secs: 60,
            price_precision: None,
        }
    }
}

/// Recent trade at a price level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelTrade {
    pub trade_id: String,
    pub price: Decimal,
    pub volume: Decimal,
    pub side: TradeSide,
    pub timestamp: DateTime<Utc>,
    /// Age in milliseconds (for fade effects)
    pub age_ms: u64,
}

/// Aggregated trade statistics at a price level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelTradeStats {
    pub price: Decimal,
    /// Total volume traded at this level
    pub total_volume: Decimal,
    /// Number of trades
    pub trade_count: usize,
    /// Buy volume
    pub buy_volume: Decimal,
    /// Sell volume
    pub sell_volume: Decimal,
    /// Most recent trade
    pub last_trade: Option<LevelTrade>,
    /// Average trade size
    pub avg_trade_size: Decimal,
    /// Time of first trade in window
    pub first_trade_time: Option<DateTime<Utc>>,
    /// Time of last trade in window
    pub last_trade_time: Option<DateTime<Utc>>,
}

/// Tracks recent trades aligned with price levels for overlay visualization
pub struct TradesByPriceLevel {
    config: TradeOverlayConfig,
    /// Trades by symbol -> price -> trades
    trades: Mutex<HashMap<String, BTreeMap<Decimal, VecDeque<LevelTrade>>>>,
}

impl TradesByPriceLevel {
    /// Create with default config
    pub fn new() -> Self {
        Self::with_config(TradeOverlayConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: TradeOverlayConfig) -> Self {
        Self {
            config,
            trades: Mutex::new(HashMap::new()),
        }
    }
    
    /// Add a trade
    pub fn add_trade(&self, trade: &TradeData) {
        let mut trades = self.trades.lock().unwrap();
        let symbol_trades = trades.entry(trade.symbol.clone()).or_default();
        
        // Round price to tick size if configured
        let price = match self.config.price_precision {
            Some(tick) if !tick.is_zero() => (trade.price / tick).floor() * tick,
            _ => trade.price,
        };
        
        let level_trades = symbol_trades.entry(price).or_default();
        
        level_trades.push_back(LevelTrade {
            trade_id: trade.trade_id.clone(),
            price: trade.price,
            volume: trade.volume,
            side: trade.side.clone(),
            timestamp: trade.timestamp,
            age_ms: 0,
        });
        
        // Trim old trades
        while level_trades.len() > self.config.max_trades_per_level {
            level_trades.pop_front();
        }
        
        // Trim old levels if too many
        while symbol_trades.len() > self.config.max_levels {
            // Remove level with oldest last trade
            if let Some(oldest_price) = symbol_trades
                .iter()
                .min_by_key(|(_, trades)| trades.back().map(|t| t.timestamp))
                .map(|(p, _)| *p)
            {
                symbol_trades.remove(&oldest_price);
            } else {
                break;
            }
        }
    }
    
    /// Get trades at a specific price level
    pub fn get_trades_at_price(&self, symbol: &str, price: Decimal) -> Vec<LevelTrade> {
        let trades = self.trades.lock().unwrap();
        let now = Utc::now();
        
        trades
            .get(symbol)
            .and_then(|t| t.get(&price))
            .map(|level_trades| {
                level_trades
                    .iter()
                    .map(|t| {
                        let mut trade = t.clone();
                        trade.age_ms = (now - t.timestamp).num_milliseconds().max(0) as u64;
                        trade
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Get aggregated stats at a price level
    pub fn get_stats_at_price(&self, symbol: &str, price: Decimal) -> Option<LevelTradeStats> {
        let trades = self.trades.lock().unwrap();
        let cutoff = Utc::now() - chrono::Duration::seconds(self.config.aggregation_window_secs as i64);
        
        trades.get(symbol).and_then(|t| t.get(&price)).map(|level_trades| {
            let recent: Vec<_> = level_trades
                .iter()
                .filter(|t| t.timestamp >= cutoff)
                .collect();
            
            let total_volume: Decimal = recent.iter().map(|t| t.volume).sum();
            let buy_volume: Decimal = recent
                .iter()
                .filter(|t| matches!(t.side, TradeSide::Buy))
                .map(|t| t.volume)
                .sum();
            let sell_volume: Decimal = recent
                .iter()
                .filter(|t| matches!(t.side, TradeSide::Sell))
                .map(|t| t.volume)
                .sum();
            
            let trade_count = recent.len();
            let avg_trade_size = if trade_count > 0 {
                total_volume / Decimal::from(trade_count)
            } else {
                Decimal::ZERO
            };
            
            let now = Utc::now();
            LevelTradeStats {
                price,
                total_volume,
                trade_count,
                buy_volume,
                sell_volume,
                last_trade: recent.last().map(|t| {
                    let mut trade = (*t).clone();
                    trade.age_ms = (now - t.timestamp).num_milliseconds().max(0) as u64;
                    trade
                }),
                avg_trade_size,
                first_trade_time: recent.first().map(|t| t.timestamp),
                last_trade_time: recent.last().map(|t| t.timestamp),
            }
        })
    }
    
    /// Get all active price levels with trades for a symbol
    pub fn get_active_levels(&self, symbol: &str) -> Vec<Decimal> {
        let trades = self.trades.lock().unwrap();
        let cutoff = Utc::now() - chrono::Duration::seconds(self.config.aggregation_window_secs as i64);
        
        trades
            .get(symbol)
            .map(|t| {
                t.iter()
                    .filter(|(_, trades)| trades.iter().any(|t| t.timestamp >= cutoff))
                    .map(|(price, _)| *price)
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Get trade overlay data for depth ladder visualization
    pub fn get_trade_overlay(&self, symbol: &str) -> Vec<LevelTradeStats> {
        let prices = self.get_active_levels(symbol);
        prices
            .into_iter()
            .filter_map(|p| self.get_stats_at_price(symbol, p))
            .collect()
    }
    
    /// Clear trades for a symbol
    pub fn clear_symbol(&self, symbol: &str) {
        self.trades.lock().unwrap().remove(symbol);
    }
    
    /// Clear all trades
    pub fn clear_all(&self) {
        self.trades.lock().unwrap().clear();
    }
}

impl Default for TradesByPriceLevel {
    fn default() -> Self {
        Self::new()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MARKET PAUSE / STALE BOOK DETECTION
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Market health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    /// Market is active and receiving updates
    Active,
    /// Market may be stale (no recent updates)
    Stale,
    /// Market appears halted or paused
    Halted,
    /// Unknown status (no data yet)
    Unknown,
}

/// Configuration for stale book detection
#[derive(Debug, Clone)]
pub struct StaleDetectionConfig {
    /// Time without updates before marking as stale (seconds)
    pub stale_threshold_secs: u64,
    /// Time without updates before marking as halted (seconds)
    pub halt_threshold_secs: u64,
}

impl Default for StaleDetectionConfig {
    fn default() -> Self {
        Self {
            stale_threshold_secs: 5,
            halt_threshold_secs: 30,
        }
    }
}

/// Tracks market health and detects stale/halted conditions
pub struct MarketHealthTracker {
    config: StaleDetectionConfig,
    /// Last update time by symbol
    last_updates: Mutex<HashMap<String, DateTime<Utc>>>,
    /// Current status by symbol
    status: Mutex<HashMap<String, MarketStatus>>,
}

impl MarketHealthTracker {
    pub fn new() -> Self {
        Self::with_config(StaleDetectionConfig::default())
    }
    
    pub fn with_config(config: StaleDetectionConfig) -> Self {
        Self {
            config,
            last_updates: Mutex::new(HashMap::new()),
            status: Mutex::new(HashMap::new()),
        }
    }
    
    /// Record an update for a symbol
    pub fn record_update(&self, symbol: &str) {
        let mut last_updates = self.last_updates.lock().unwrap();
        last_updates.insert(symbol.to_string(), Utc::now());
        
        let mut status = self.status.lock().unwrap();
        status.insert(symbol.to_string(), MarketStatus::Active);
    }
    
    /// Check current status for a symbol
    pub fn check_status(&self, symbol: &str) -> MarketStatus {
        let last_updates = self.last_updates.lock().unwrap();
        
        match last_updates.get(symbol) {
            None => MarketStatus::Unknown,
            Some(last_update) => {
                let elapsed = (Utc::now() - *last_update).num_seconds() as u64;
                
                if elapsed >= self.config.halt_threshold_secs {
                    MarketStatus::Halted
                } else if elapsed >= self.config.stale_threshold_secs {
                    MarketStatus::Stale
                } else {
                    MarketStatus::Active
                }
            }
        }
    }
    
    /// Get time since last update (in milliseconds)
    pub fn get_time_since_update(&self, symbol: &str) -> Option<u64> {
        let last_updates = self.last_updates.lock().unwrap();
        last_updates
            .get(symbol)
            .map(|t| (Utc::now() - *t).num_milliseconds().max(0) as u64)
    }
    
    /// Update all statuses (call periodically)
    pub fn tick(&self) -> HashMap<String, MarketStatus> {
        let last_updates = self.last_updates.lock().unwrap();
        let mut status = self.status.lock().unwrap();
        
        for (symbol, last_update) in last_updates.iter() {
            let elapsed = (Utc::now() - *last_update).num_seconds() as u64;
            
            let new_status = if elapsed >= self.config.halt_threshold_secs {
                MarketStatus::Halted
            } else if elapsed >= self.config.stale_threshold_secs {
                MarketStatus::Stale
            } else {
                MarketStatus::Active
            };
            
            status.insert(symbol.clone(), new_status);
        }
        
        status.clone()
    }
}

impl Default for MarketHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use crate::data::PriceLevel;
    
    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }
    
    #[test]
    fn test_flow_tracker_detects_new_level() {
        let tracker = OrderFlowTracker::new();
        
        // Create an order book
        let mut book = OrderBook::new("BTC/USD");
        book.bids.insert(dec("50000"), PriceLevel {
            price: dec("50000"),
            volume: dec("1.0"),
            timestamp: Utc::now(),
        });
        
        // First track - no events (no previous snapshot)
        let events = tracker.track_update(&book);
        assert!(events.is_empty());
        
        // Add a new level
        book.bids.insert(dec("49900"), PriceLevel {
            price: dec("49900"),
            volume: dec("2.0"),
            timestamp: Utc::now(),
        });
        
        let events = tracker.track_update(&book);
        assert!(!events.is_empty());
        assert!(events.iter().any(|e| matches!(e.event_type, FlowEventType::LevelAdded)));
    }
    
    #[test]
    fn test_flow_tracker_detects_large_order() {
        let mut config = OrderFlowConfig::default();
        config.large_order_threshold = dec("5.0");
        let tracker = OrderFlowTracker::with_config(config);
        
        let mut book = OrderBook::new("BTC/USD");
        book.bids.insert(dec("50000"), PriceLevel {
            price: dec("50000"),
            volume: dec("1.0"),
            timestamp: Utc::now(),
        });
        
        // Initial snapshot
        tracker.track_update(&book);
        
        // Increase to large order
        book.bids.get_mut(&dec("50000")).unwrap().volume = dec("10.0");
        
        let events = tracker.track_update(&book);
        assert!(events.iter().any(|e| matches!(e.event_type, FlowEventType::LargeOrderAppeared)));
    }
    
    #[test]
    fn test_trades_by_price_level() {
        let tracker = TradesByPriceLevel::new();
        
        let trade = TradeData {
            symbol: "BTC/USD".to_string(),
            price: dec("50000"),
            volume: dec("0.5"),
            side: TradeSide::Buy,
            timestamp: Utc::now(),
            trade_id: "trade1".to_string(),
        };
        
        tracker.add_trade(&trade);
        
        let trades = tracker.get_trades_at_price("BTC/USD", dec("50000"));
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].volume, dec("0.5"));
    }
    
    #[test]
    fn test_market_health_tracker() {
        let config = StaleDetectionConfig {
            stale_threshold_secs: 1,
            halt_threshold_secs: 2,
        };
        let tracker = MarketHealthTracker::with_config(config);
        
        tracker.record_update("BTC/USD");
        assert_eq!(tracker.check_status("BTC/USD"), MarketStatus::Active);
        
        // Unknown for untracked symbol
        assert_eq!(tracker.check_status("ETH/USD"), MarketStatus::Unknown);
    }
}

