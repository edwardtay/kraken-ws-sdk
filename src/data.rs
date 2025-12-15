//! Data models for market data structures

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Ticker data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickerData {
    pub symbol: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last_price: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl fmt::Display for TickerData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Ticker[{}]: bid={}, ask={}, last={}, vol={} @ {}",
            self.symbol, self.bid, self.ask, self.last_price, self.volume, self.timestamp
        )
    }
}

/// Order book update structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderBookUpdate {
    pub symbol: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub timestamp: DateTime<Utc>,
    pub checksum: Option<u32>,
}

impl fmt::Display for OrderBookUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OrderBook[{}]: {} bids, {} asks @ {}",
            self.symbol, self.bids.len(), self.asks.len(), self.timestamp
        )
    }
}

/// Trade data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeData {
    pub symbol: String,
    pub price: Decimal,
    pub volume: Decimal,
    pub side: TradeSide,
    pub timestamp: DateTime<Utc>,
    pub trade_id: String,
}

impl fmt::Display for TradeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Trade[{}]: {:?} {} @ {} (ID: {}) @ {}",
            self.symbol, self.side, self.volume, self.price, self.trade_id, self.timestamp
        )
    }
}

/// Price level in order book
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceLevel {
    pub price: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl fmt::Display for PriceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.volume, self.price)
    }
}

/// OHLC (Open, High, Low, Close) data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OHLCData {
    pub symbol: String,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
    pub interval: String,
}

impl fmt::Display for OHLCData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OHLC[{}:{}]: O={} H={} L={} C={} V={} @ {}",
            self.symbol, self.interval, self.open, self.high, self.low, self.close, self.volume, self.timestamp
        )
    }
}

/// Trade side enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Data type enumeration for event dispatching
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataType {
    Ticker,
    OrderBook,
    Trade,
    OHLC,
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub endpoint: String,
    pub reconnect_config: ReconnectConfig,
    pub buffer_size: usize,
    pub timeout: std::time::Duration,
}

impl ClientConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.endpoint.is_empty() {
            return Err("Endpoint cannot be empty".to_string());
        }
        
        if !self.endpoint.starts_with("ws://") && !self.endpoint.starts_with("wss://") {
            return Err("Endpoint must be a valid WebSocket URL".to_string());
        }
        
        if self.buffer_size == 0 {
            return Err("Buffer size must be greater than 0".to_string());
        }
        
        if self.timeout.as_secs() == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }
        
        self.reconnect_config.validate()?;
        
        Ok(())
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_secret: None,
            endpoint: "wss://ws.kraken.com".to_string(),
            reconnect_config: ReconnectConfig::default(),
            buffer_size: 1024,
            timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// Reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    pub max_attempts: u32,
    pub initial_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
    pub backoff_multiplier: f64,
}

impl ReconnectConfig {
    /// Validate reconnection configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_attempts == 0 {
            return Err("Max attempts must be greater than 0".to_string());
        }
        
        if self.initial_delay.as_millis() == 0 {
            return Err("Initial delay must be greater than 0".to_string());
        }
        
        if self.max_delay < self.initial_delay {
            return Err("Max delay must be greater than or equal to initial delay".to_string());
        }
        
        if self.backoff_multiplier <= 1.0 {
            return Err("Backoff multiplier must be greater than 1.0".to_string());
        }
        
        Ok(())
    }
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub endpoint: String,
    pub timeout: std::time::Duration,
    pub ping_interval: std::time::Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            endpoint: "wss://ws.kraken.com".to_string(),
            timeout: std::time::Duration::from_secs(30),
            ping_interval: std::time::Duration::from_secs(30),
        }
    }
}

/// Connection state enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Channel specification for subscriptions
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub name: String,
    pub symbol: Option<String>,
    pub interval: Option<String>,
}

impl Channel {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            symbol: None,
            interval: None,
        }
    }
    
    pub fn with_symbol(mut self, symbol: &str) -> Self {
        self.symbol = Some(symbol.to_string());
        self
    }
    
    pub fn with_interval(mut self, interval: &str) -> Self {
        self.interval = Some(interval.to_string());
        self
    }
}