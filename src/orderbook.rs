//! Order book state management

use crate::{
    data::{OrderBookUpdate, PriceLevel},
    error::ParseError,
};
use rust_decimal::Decimal;
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