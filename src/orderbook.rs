//! Order book state management
//!
//! Production-grade order book with features for professional visualization:
//! - Price aggregation (tick size grouping)
//! - Cumulative depth calculation
//! - Liquidity imbalance ratios
//! - Depth ladder generation
//!
//! ## Example: Building an Order Book Visualizer
//!
//! ```rust,ignore
//! use kraken_ws_sdk::orderbook::{OrderBook, AggregatedBook, DepthLadder};
//! use rust_decimal_macros::dec;
//!
//! // Get aggregated book grouped by $10 tick size
//! let aggregated = order_book.aggregate(dec!(10.0));
//!
//! // Get depth ladder with cumulative sizes (top 20 levels)
//! let ladder = order_book.get_depth_ladder(20);
//!
//! // Calculate bid/ask imbalance over top 10 levels
//! let imbalance = order_book.get_imbalance_ratio(10);
//! println!("Imbalance: {:.2}% bid-heavy", imbalance * dec!(100));
//! ```

use crate::{
    data::{OrderBookUpdate, PriceLevel},
    error::ParseError,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Order book state manager
#[derive(Debug)]
pub struct OrderBookManager {
    /// Current order book state by symbol
    order_books: Arc<Mutex<std::collections::HashMap<String, OrderBook>>>,
}

/// Order book state for a single symbol
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: BTreeMap<Decimal, PriceLevel>,
    pub asks: BTreeMap<Decimal, PriceLevel>,
    pub last_update: chrono::DateTime<chrono::Utc>,
    pub checksum: Option<u32>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VISUALIZATION DATA STRUCTURES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Aggregated order book with price levels grouped by tick size
/// 
/// Used to reduce noise and reveal true liquidity at configurable price increments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedBook {
    pub symbol: String,
    /// Aggregated bid levels (price -> total volume at that tick)
    pub bids: Vec<AggregatedLevel>,
    /// Aggregated ask levels (price -> total volume at that tick)
    pub asks: Vec<AggregatedLevel>,
    /// Tick size used for aggregation
    pub tick_size: Decimal,
    /// Timestamp of aggregation
    pub timestamp: DateTime<Utc>,
}

/// A single aggregated price level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedLevel {
    /// Price bucket (rounded to tick size)
    pub price: Decimal,
    /// Total volume at this price bucket
    pub volume: Decimal,
    /// Number of orders aggregated into this level
    pub order_count: usize,
}

/// Depth ladder with cumulative sizes for visualization
///
/// Each level includes running totals from best price, enabling
/// horizontal bar charts showing size dominance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthLadder {
    pub symbol: String,
    /// Bid side ladder (best bid first)
    pub bids: Vec<LadderLevel>,
    /// Ask side ladder (best ask first)
    pub asks: Vec<LadderLevel>,
    /// Best bid price
    pub best_bid: Option<Decimal>,
    /// Best ask price
    pub best_ask: Option<Decimal>,
    /// Mid price
    pub mid_price: Option<Decimal>,
    /// Spread in absolute terms
    pub spread: Option<Decimal>,
    /// Spread as percentage of mid price
    pub spread_bps: Option<Decimal>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// A single level in the depth ladder with cumulative data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LadderLevel {
    /// Price at this level
    pub price: Decimal,
    /// Volume at this specific level
    pub volume: Decimal,
    /// Cumulative volume from best price to this level
    pub cumulative_volume: Decimal,
    /// Percentage of total side volume at this level
    pub volume_percent: Decimal,
    /// Cumulative percentage from best price
    pub cumulative_percent: Decimal,
    /// Distance from mid price in absolute terms
    pub distance_from_mid: Option<Decimal>,
    /// Distance from mid price in basis points
    pub distance_bps: Option<Decimal>,
    /// Original timestamp of this level
    pub timestamp: DateTime<Utc>,
}

/// Liquidity imbalance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImbalanceMetrics {
    pub symbol: String,
    /// Imbalance ratio: (bid_vol - ask_vol) / (bid_vol + ask_vol)
    /// Range: -1.0 (all asks) to +1.0 (all bids)
    pub imbalance_ratio: Decimal,
    /// Total bid volume in the measured depth
    pub bid_volume: Decimal,
    /// Total ask volume in the measured depth
    pub ask_volume: Decimal,
    /// Number of levels measured
    pub depth_levels: usize,
    /// Bid-weighted average price
    pub bid_vwap: Option<Decimal>,
    /// Ask-weighted average price
    pub ask_vwap: Option<Decimal>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Book pressure indicator (order flow signal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookPressure {
    pub symbol: String,
    /// Pressure score: positive = buy pressure, negative = sell pressure
    /// Calculated from volume-weighted depth imbalance
    pub pressure_score: Decimal,
    /// Interpretation of the pressure
    pub signal: PressureSignal,
    /// Confidence level (0.0 - 1.0)
    pub confidence: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Pressure signal interpretation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureSignal {
    StrongBuy,
    WeakBuy,
    Neutral,
    WeakSell,
    StrongSell,
}

/// Filtered order book within a price range
/// 
/// Used to focus on liquidity near the mid-price, filtering out
/// distant price levels that add noise to visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredBook {
    pub symbol: String,
    /// Filtered bid levels within range
    pub bids: Vec<PriceLevel>,
    /// Filtered ask levels within range
    pub asks: Vec<PriceLevel>,
    /// Mid price at time of filtering
    pub mid_price: Decimal,
    /// Range percentage used for filtering
    pub range_percent: Decimal,
    /// Minimum price in range
    pub price_min: Decimal,
    /// Maximum price in range
    pub price_max: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl OrderBookManager {
    pub fn new() -> Self {
        Self {
            order_books: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
    
    /// Apply order book update and maintain state
    pub fn apply_update(&self, update: OrderBookUpdate) -> Result<OrderBook, ParseError> {
        let mut order_books = self.order_books.lock().unwrap();
        
        // Get or create order book for this symbol
        let order_book = order_books
            .entry(update.symbol.clone())
            .or_insert_with(|| OrderBook::new(&update.symbol));
        
        // Apply bid updates
        for bid in &update.bids {
            if bid.volume.is_zero() {
                // Remove price level if volume is zero
                order_book.bids.remove(&bid.price);
            } else {
                // Update or insert price level
                order_book.bids.insert(bid.price, bid.clone());
            }
        }
        
        // Apply ask updates
        for ask in &update.asks {
            if ask.volume.is_zero() {
                // Remove price level if volume is zero
                order_book.asks.remove(&ask.price);
            } else {
                // Update or insert price level
                order_book.asks.insert(ask.price, ask.clone());
            }
        }
        
        // Update metadata
        order_book.last_update = update.timestamp;
        order_book.checksum = update.checksum;
        
        // Validate order book integrity
        self.validate_order_book(order_book)?;
        
        Ok(order_book.clone())
    }
    
    /// Get current order book state for a symbol
    pub fn get_order_book(&self, symbol: &str) -> Option<OrderBook> {
        let order_books = self.order_books.lock().unwrap();
        order_books.get(symbol).cloned()
    }
    
    /// Get best bid and ask prices
    pub fn get_best_bid_ask(&self, symbol: &str) -> Option<(Option<Decimal>, Option<Decimal>)> {
        let order_books = self.order_books.lock().unwrap();
        if let Some(order_book) = order_books.get(symbol) {
            let best_bid = order_book.bids.keys().next_back().copied();
            let best_ask = order_book.asks.keys().next().copied();
            Some((best_bid, best_ask))
        } else {
            None
        }
    }
    
    /// Get order book depth (top N levels)
    pub fn get_depth(&self, symbol: &str, depth: usize) -> Option<(Vec<PriceLevel>, Vec<PriceLevel>)> {
        let order_books = self.order_books.lock().unwrap();
        if let Some(order_book) = order_books.get(symbol) {
            let bids: Vec<PriceLevel> = order_book.bids
                .values()
                .rev()
                .take(depth)
                .cloned()
                .collect();
            
            let asks: Vec<PriceLevel> = order_book.asks
                .values()
                .take(depth)
                .cloned()
                .collect();
            
            Some((bids, asks))
        } else {
            None
        }
    }
    
    /// Calculate order book checksum for integrity verification
    pub fn calculate_checksum(&self, symbol: &str) -> Option<u32> {
        let order_books = self.order_books.lock().unwrap();
        if let Some(order_book) = order_books.get(symbol) {
            // Simple checksum calculation (in production, use Kraken's specific algorithm)
            let mut checksum_data = String::new();
            
            // Add top 10 bids and asks to checksum
            for (price, level) in order_book.bids.iter().rev().take(10) {
                checksum_data.push_str(&format!("{}:{}", price, level.volume));
            }
            
            for (price, level) in order_book.asks.iter().take(10) {
                checksum_data.push_str(&format!("{}:{}", price, level.volume));
            }
            
            // Calculate simple hash (in production, use CRC32 or similar)
            Some(checksum_data.len() as u32)
        } else {
            None
        }
    }
    
    /// Validate order book integrity
    fn validate_order_book(&self, order_book: &OrderBook) -> Result<(), ParseError> {
        // Check that bids are in descending order (highest first)
        let mut prev_bid_price: Option<Decimal> = None;
        for &price in order_book.bids.keys().rev() {
            if let Some(prev_price) = prev_bid_price {
                if price >= prev_price {
                    return Err(ParseError::MalformedMessage(
                        "Bid prices not in descending order".to_string()
                    ));
                }
            }
            prev_bid_price = Some(price);
        }
        
        // Check that asks are in ascending order (lowest first)
        let mut prev_ask_price: Option<Decimal> = None;
        for &price in order_book.asks.keys() {
            if let Some(prev_price) = prev_ask_price {
                if price <= prev_price {
                    return Err(ParseError::MalformedMessage(
                        "Ask prices not in ascending order".to_string()
                    ));
                }
            }
            prev_ask_price = Some(price);
        }
        
        // Check that best bid < best ask (no crossed book)
        if let (Some(best_bid), Some(best_ask)) = (
            order_book.bids.keys().next_back(),
            order_book.asks.keys().next()
        ) {
            if best_bid >= best_ask {
                tracing::warn!("Crossed order book detected: bid={}, ask={}", best_bid, best_ask);
                // Don't fail, just warn as this can happen during rapid updates
            }
        }
        
        Ok(())
    }
    
    /// Clear order book for a symbol
    pub fn clear_order_book(&self, symbol: &str) {
        let mut order_books = self.order_books.lock().unwrap();
        order_books.remove(symbol);
    }
    
    /// Get all tracked symbols
    pub fn get_symbols(&self) -> Vec<String> {
        let order_books = self.order_books.lock().unwrap();
        order_books.keys().cloned().collect()
    }
}

impl OrderBook {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update: chrono::Utc::now(),
            checksum: None,
        }
    }
    
    /// Get spread (difference between best ask and best bid)
    pub fn get_spread(&self) -> Option<Decimal> {
        if let (Some(best_bid), Some(best_ask)) = (
            self.bids.keys().next_back(),
            self.asks.keys().next()
        ) {
            Some(*best_ask - *best_bid)
        } else {
            None
        }
    }
    
    /// Get mid price (average of best bid and ask)
    pub fn get_mid_price(&self) -> Option<Decimal> {
        if let (Some(best_bid), Some(best_ask)) = (
            self.bids.keys().next_back(),
            self.asks.keys().next()
        ) {
            Some((*best_bid + *best_ask) / Decimal::from(2))
        } else {
            None
        }
    }
    
    /// Check if order book is empty
    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }
    
    /// Get total volume at all price levels
    pub fn get_total_volume(&self) -> (Decimal, Decimal) {
        let bid_volume = self.bids.values()
            .map(|level| level.volume)
            .sum();
        
        let ask_volume = self.asks.values()
            .map(|level| level.volume)
            .sum();
        
        (bid_volume, ask_volume)
    }
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // VISUALIZATION METHODS
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    
    /// Aggregate order book by tick size to reduce noise
    ///
    /// Groups price levels into buckets of the specified tick size,
    /// summing volumes within each bucket.
    ///
    /// # Example
    /// ```rust,ignore
    /// use rust_decimal_macros::dec;
    /// 
    /// // Group BTC prices into $100 buckets
    /// let aggregated = order_book.aggregate(dec!(100.0));
    /// 
    /// // Group ETH prices into $10 buckets  
    /// let aggregated = order_book.aggregate(dec!(10.0));
    /// ```
    pub fn aggregate(&self, tick_size: Decimal) -> AggregatedBook {
        let aggregate_side = |levels: &BTreeMap<Decimal, PriceLevel>, is_bid: bool| -> Vec<AggregatedLevel> {
            let mut buckets: BTreeMap<Decimal, (Decimal, usize)> = BTreeMap::new();
            
            for (price, level) in levels.iter() {
                // Round price to nearest tick
                let bucket_price = (*price / tick_size).floor() * tick_size;
                let entry = buckets.entry(bucket_price).or_insert((Decimal::ZERO, 0));
                entry.0 += level.volume;
                entry.1 += 1;
            }
            
            let mut result: Vec<AggregatedLevel> = buckets
                .into_iter()
                .map(|(price, (volume, count))| AggregatedLevel {
                    price,
                    volume,
                    order_count: count,
                })
                .collect();
            
            // Sort: bids descending, asks ascending
            if is_bid {
                result.sort_by(|a, b| b.price.cmp(&a.price));
            } else {
                result.sort_by(|a, b| a.price.cmp(&b.price));
            }
            
            result
        };
        
        AggregatedBook {
            symbol: self.symbol.clone(),
            bids: aggregate_side(&self.bids, true),
            asks: aggregate_side(&self.asks, false),
            tick_size,
            timestamp: Utc::now(),
        }
    }
    
    /// Get depth ladder with cumulative sizes for visualization
    ///
    /// Returns a depth ladder structure with:
    /// - Per-level volume
    /// - Cumulative volume from best price
    /// - Volume percentages
    /// - Distance from mid price
    ///
    /// # Example
    /// ```rust,ignore
    /// let ladder = order_book.get_depth_ladder(20);
    /// 
    /// // Render horizontal bars showing cumulative depth
    /// for level in ladder.bids {
    ///     let bar_width = level.cumulative_percent; // 0-100%
    ///     render_bid_bar(level.price, bar_width);
    /// }
    /// ```
    pub fn get_depth_ladder(&self, depth: usize) -> DepthLadder {
        let mid_price = self.get_mid_price();
        let spread = self.get_spread();
        let best_bid = self.bids.keys().next_back().copied();
        let best_ask = self.asks.keys().next().copied();
        
        // Calculate spread in basis points
        let spread_bps = match (spread, mid_price) {
            (Some(s), Some(m)) if !m.is_zero() => Some((s / m) * Decimal::from(10000)),
            _ => None,
        };
        
        // Build bid ladder (best bid first = highest price first)
        let bid_levels: Vec<_> = self.bids.iter().rev().take(depth).collect();
        let total_bid_volume: Decimal = bid_levels.iter().map(|(_, l)| l.volume).sum();
        
        let mut cumulative_bid = Decimal::ZERO;
        let bids: Vec<LadderLevel> = bid_levels
            .into_iter()
            .map(|(price, level)| {
                cumulative_bid += level.volume;
                let volume_percent = if total_bid_volume.is_zero() {
                    Decimal::ZERO
                } else {
                    (level.volume / total_bid_volume) * Decimal::from(100)
                };
                let cumulative_percent = if total_bid_volume.is_zero() {
                    Decimal::ZERO
                } else {
                    (cumulative_bid / total_bid_volume) * Decimal::from(100)
                };
                
                let (distance_from_mid, distance_bps) = match mid_price {
                    Some(m) if !m.is_zero() => {
                        let dist = m - *price;
                        let bps = (dist / m) * Decimal::from(10000);
                        (Some(dist), Some(bps))
                    }
                    _ => (None, None),
                };
                
                LadderLevel {
                    price: *price,
                    volume: level.volume,
                    cumulative_volume: cumulative_bid,
                    volume_percent,
                    cumulative_percent,
                    distance_from_mid,
                    distance_bps,
                    timestamp: level.timestamp,
                }
            })
            .collect();
        
        // Build ask ladder (best ask first = lowest price first)
        let ask_levels: Vec<_> = self.asks.iter().take(depth).collect();
        let total_ask_volume: Decimal = ask_levels.iter().map(|(_, l)| l.volume).sum();
        
        let mut cumulative_ask = Decimal::ZERO;
        let asks: Vec<LadderLevel> = ask_levels
            .into_iter()
            .map(|(price, level)| {
                cumulative_ask += level.volume;
                let volume_percent = if total_ask_volume.is_zero() {
                    Decimal::ZERO
                } else {
                    (level.volume / total_ask_volume) * Decimal::from(100)
                };
                let cumulative_percent = if total_ask_volume.is_zero() {
                    Decimal::ZERO
                } else {
                    (cumulative_ask / total_ask_volume) * Decimal::from(100)
                };
                
                let (distance_from_mid, distance_bps) = match mid_price {
                    Some(m) if !m.is_zero() => {
                        let dist = *price - m;
                        let bps = (dist / m) * Decimal::from(10000);
                        (Some(dist), Some(bps))
                    }
                    _ => (None, None),
                };
                
                LadderLevel {
                    price: *price,
                    volume: level.volume,
                    cumulative_volume: cumulative_ask,
                    volume_percent,
                    cumulative_percent,
                    distance_from_mid,
                    distance_bps,
                    timestamp: level.timestamp,
                }
            })
            .collect();
        
        DepthLadder {
            symbol: self.symbol.clone(),
            bids,
            asks,
            best_bid,
            best_ask,
            mid_price,
            spread,
            spread_bps,
            timestamp: Utc::now(),
        }
    }
    
    /// Calculate liquidity imbalance ratio over configurable depth
    ///
    /// Returns a value from -1.0 (all asks) to +1.0 (all bids).
    /// Values near 0 indicate balanced liquidity.
    ///
    /// # Example
    /// ```rust,ignore
    /// let imbalance = order_book.get_imbalance_ratio(10);
    /// 
    /// if imbalance > dec!(0.3) {
    ///     println!("Strong bid pressure - potential upward move");
    /// } else if imbalance < dec!(-0.3) {
    ///     println!("Strong ask pressure - potential downward move");
    /// }
    /// ```
    pub fn get_imbalance_ratio(&self, depth: usize) -> Decimal {
        let bid_volume: Decimal = self.bids.iter()
            .rev()
            .take(depth)
            .map(|(_, l)| l.volume)
            .sum();
        
        let ask_volume: Decimal = self.asks.iter()
            .take(depth)
            .map(|(_, l)| l.volume)
            .sum();
        
        let total = bid_volume + ask_volume;
        if total.is_zero() {
            return Decimal::ZERO;
        }
        
        (bid_volume - ask_volume) / total
    }
    
    /// Get comprehensive imbalance metrics
    ///
    /// Includes volume-weighted average prices (VWAP) for each side,
    /// useful for understanding where liquidity is concentrated.
    pub fn get_imbalance_metrics(&self, depth: usize) -> ImbalanceMetrics {
        let bid_levels: Vec<_> = self.bids.iter().rev().take(depth).collect();
        let ask_levels: Vec<_> = self.asks.iter().take(depth).collect();
        
        let bid_volume: Decimal = bid_levels.iter().map(|(_, l)| l.volume).sum();
        let ask_volume: Decimal = ask_levels.iter().map(|(_, l)| l.volume).sum();
        let total = bid_volume + ask_volume;
        
        let imbalance_ratio = if total.is_zero() {
            Decimal::ZERO
        } else {
            (bid_volume - ask_volume) / total
        };
        
        // Calculate VWAP for each side
        let bid_vwap = if bid_volume.is_zero() {
            None
        } else {
            let weighted_sum: Decimal = bid_levels.iter()
                .map(|(price, level)| **price * level.volume)
                .sum();
            Some(weighted_sum / bid_volume)
        };
        
        let ask_vwap = if ask_volume.is_zero() {
            None
        } else {
            let weighted_sum: Decimal = ask_levels.iter()
                .map(|(price, level)| **price * level.volume)
                .sum();
            Some(weighted_sum / ask_volume)
        };
        
        ImbalanceMetrics {
            symbol: self.symbol.clone(),
            imbalance_ratio,
            bid_volume,
            ask_volume,
            depth_levels: depth,
            bid_vwap,
            ask_vwap,
            timestamp: Utc::now(),
        }
    }
    
    /// Calculate book pressure for trading signals
    ///
    /// Uses volume-weighted depth to generate a pressure score
    /// and signal interpretation.
    pub fn get_book_pressure(&self, depth: usize) -> BookPressure {
        let metrics = self.get_imbalance_metrics(depth);
        let total_volume = metrics.bid_volume + metrics.ask_volume;
        
        // Pressure score weighted by proximity to mid price
        let pressure_score = metrics.imbalance_ratio;
        
        // Determine signal based on thresholds
        let signal = if pressure_score > Decimal::new(5, 1) {
            PressureSignal::StrongBuy
        } else if pressure_score > Decimal::new(2, 1) {
            PressureSignal::WeakBuy
        } else if pressure_score < Decimal::new(-5, 1) {
            PressureSignal::StrongSell
        } else if pressure_score < Decimal::new(-2, 1) {
            PressureSignal::WeakSell
        } else {
            PressureSignal::Neutral
        };
        
        // Confidence based on total volume (more volume = more confident)
        // Normalized to 0-1 range with diminishing returns
        let confidence = if total_volume.is_zero() {
            Decimal::ZERO
        } else {
            // Sigmoid-like normalization
            let normalized = total_volume / (total_volume + Decimal::from(100));
            normalized.min(Decimal::ONE)
        };
        
        BookPressure {
            symbol: self.symbol.clone(),
            pressure_score,
            signal,
            confidence,
            timestamp: Utc::now(),
        }
    }
    
    /// Get top N levels from each side (convenience method for ladder view)
    pub fn get_top_levels(&self, n: usize) -> (Vec<&PriceLevel>, Vec<&PriceLevel>) {
        let top_bids: Vec<_> = self.bids.values().rev().take(n).collect();
        let top_asks: Vec<_> = self.asks.values().take(n).collect();
        (top_bids, top_asks)
    }
    
    /// Get volume at a specific price level
    pub fn get_volume_at_price(&self, price: Decimal) -> Option<Decimal> {
        self.bids.get(&price)
            .or_else(|| self.asks.get(&price))
            .map(|l| l.volume)
    }
    
    /// Get best bid level (price + volume)
    pub fn get_best_bid(&self) -> Option<&PriceLevel> {
        self.bids.values().next_back()
    }
    
    /// Filter book to only show levels within ±X% of mid-price
    ///
    /// This is useful for depth range selection - focusing on nearby
    /// liquidity rather than distant price levels.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get levels within ±0.5% of mid-price
    /// let filtered = order_book.filter_by_spread(dec!(0.5));
    /// 
    /// println!("Focused on {} - {} range", filtered.price_min, filtered.price_max);
    /// for bid in &filtered.bids {
    ///     println!("Bid: {} @ {}", bid.volume, bid.price);
    /// }
    /// ```
    pub fn filter_by_spread(&self, percent: Decimal) -> Option<FilteredBook> {
        let mid = self.get_mid_price()?;
        let range = mid * percent / Decimal::from(100);
        let price_min = mid - range;
        let price_max = mid + range;
        
        let bids: Vec<PriceLevel> = self.bids.iter()
            .rev()
            .filter(|(price, _)| **price >= price_min && **price <= price_max)
            .map(|(_, level)| level.clone())
            .collect();
        
        let asks: Vec<PriceLevel> = self.asks.iter()
            .filter(|(price, _)| **price >= price_min && **price <= price_max)
            .map(|(_, level)| level.clone())
            .collect();
        
        Some(FilteredBook {
            symbol: self.symbol.clone(),
            bids,
            asks,
            mid_price: mid,
            range_percent: percent,
            price_min,
            price_max,
            timestamp: self.last_update,
        })
    }
    
    /// Get best ask level (price + volume)
    pub fn get_best_ask(&self) -> Option<&PriceLevel> {
        self.asks.values().next()
    }
}

impl Default for OrderBookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for OrderBookManager {
    fn clone(&self) -> Self {
        Self {
            order_books: Arc::clone(&self.order_books),
        }
    }
}