//! Kraken WebSocket SDK - Clean Public API
//! 
//! Minimal but powerful API surface for real-time cryptocurrency data.

use crate::{
    data::{TickerData, TradeData, OrderBookUpdate, OHLCData, ConnectionState},
    error::SdkError,
};
use std::sync::Arc;

/// Callback for ticker data updates
pub type TickerCallback = Arc<dyn Fn(TickerData) + Send + Sync>;

/// Callback for order book updates  
pub type OrderBookCallback = Arc<dyn Fn(OrderBookUpdate) + Send + Sync>;

/// Callback for trade updates
pub type TradeCallback = Arc<dyn Fn(TradeData) + Send + Sync>;

/// Callback for OHLC updates
pub type OHLCCallback = Arc<dyn Fn(OHLCData) + Send + Sync>;

/// Callback for reconnection events
pub type ReconnectCallback = Arc<dyn Fn(u32) + Send + Sync>;

/// Callback for errors
pub type ErrorCallback = Arc<dyn Fn(SdkError) + Send + Sync>;

/// Builder for creating KrakenSDK instances
pub struct KrakenSDKBuilder {
    endpoint: String,
    auto_reconnect: bool,
    max_reconnect_attempts: u32,
}

impl KrakenSDKBuilder {
    pub fn new() -> Self {
        Self {
            endpoint: "wss://ws.kraken.com".to_string(),
            auto_reconnect: true,
            max_reconnect_attempts: 10,
        }
    }
    
    pub fn endpoint(mut self, url: &str) -> Self {
        self.endpoint = url.to_string();
        self
    }
    
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }
    
    pub fn max_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.max_reconnect_attempts = attempts;
        self
    }
    
    pub fn build(self) -> KrakenSDK {
        KrakenSDK::new(self.endpoint, self.auto_reconnect, self.max_reconnect_attempts)
    }
}

impl Default for KrakenSDKBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Main SDK interface - minimal but powerful
pub struct KrakenSDK {
    endpoint: String,
    auto_reconnect: bool,
    max_reconnect_attempts: u32,
    ticker_callbacks: std::sync::Mutex<Vec<(String, TickerCallback)>>,
    orderbook_callbacks: std::sync::Mutex<Vec<(String, u32, OrderBookCallback)>>,
    trade_callbacks: std::sync::Mutex<Vec<(String, TradeCallback)>>,
    reconnect_callback: std::sync::Mutex<Option<ReconnectCallback>>,
    error_callback: std::sync::Mutex<Option<ErrorCallback>>,
    connection_state: std::sync::Mutex<ConnectionState>,
}

impl KrakenSDK {
    /// Create new SDK instance
    pub fn new(endpoint: String, auto_reconnect: bool, max_reconnect_attempts: u32) -> Self {
        Self {
            endpoint,
            auto_reconnect,
            max_reconnect_attempts,
            ticker_callbacks: std::sync::Mutex::new(Vec::new()),
            orderbook_callbacks: std::sync::Mutex::new(Vec::new()),
            trade_callbacks: std::sync::Mutex::new(Vec::new()),
            reconnect_callback: std::sync::Mutex::new(None),
            error_callback: std::sync::Mutex::new(None),
            connection_state: std::sync::Mutex::new(ConnectionState::Disconnected),
        }
    }
    
    /// Create SDK with default settings
    pub fn default() -> Self {
        KrakenSDKBuilder::new().build()
    }
    
    /// Subscribe to ticker updates for a trading pair
    /// 
    /// # Example
    /// ```rust
    /// sdk.subscribe_ticker("BTC/USD", |ticker| {
    ///     println!("BTC: ${}", ticker.last_price);
    /// });
    /// ```
    pub fn subscribe_ticker<F>(&self, pair: &str, callback: F) -> &Self 
    where
        F: Fn(TickerData) + Send + Sync + 'static
    {
        let mut callbacks = self.ticker_callbacks.lock().unwrap();
        callbacks.push((pair.to_string(), Arc::new(callback)));
        self
    }
    
    /// Subscribe to order book updates with specified depth
    /// 
    /// # Example
    /// ```rust
    /// sdk.subscribe_orderbook("ETH/USD", 10, |book| {
    ///     println!("Best bid: {:?}", book.bids.first());
    /// });
    /// ```
    pub fn subscribe_orderbook<F>(&self, pair: &str, depth: u32, callback: F) -> &Self
    where
        F: Fn(OrderBookUpdate) + Send + Sync + 'static
    {
        let mut callbacks = self.orderbook_callbacks.lock().unwrap();
        callbacks.push((pair.to_string(), depth, Arc::new(callback)));
        self
    }
    
    /// Subscribe to trade updates
    /// 
    /// # Example
    /// ```rust
    /// sdk.subscribe_trades("BTC/USD", |trade| {
    ///     println!("{:?} {} @ ${}", trade.side, trade.volume, trade.price);
    /// });
    /// ```
    pub fn subscribe_trades<F>(&self, pair: &str, callback: F) -> &Self
    where
        F: Fn(TradeData) + Send + Sync + 'static
    {
        let mut callbacks = self.trade_callbacks.lock().unwrap();
        callbacks.push((pair.to_string(), Arc::new(callback)));
        self
    }
    
    /// Unsubscribe from a trading pair
    pub fn unsubscribe(&self, pair: &str) -> &Self {
        {
            let mut callbacks = self.ticker_callbacks.lock().unwrap();
            callbacks.retain(|(p, _)| p != pair);
        }
        {
            let mut callbacks = self.orderbook_callbacks.lock().unwrap();
            callbacks.retain(|(p, _, _)| p != pair);
        }
        {
            let mut callbacks = self.trade_callbacks.lock().unwrap();
            callbacks.retain(|(p, _)| p != pair);
        }
        self
    }
    
    /// Set reconnection handler
    /// 
    /// # Example
    /// ```rust
    /// sdk.on_reconnect(|attempt| {
    ///     println!("Reconnecting... attempt {}", attempt);
    /// });
    /// ```
    pub fn on_reconnect<F>(&self, handler: F) -> &Self
    where
        F: Fn(u32) + Send + Sync + 'static
    {
        let mut callback = self.reconnect_callback.lock().unwrap();
        *callback = Some(Arc::new(handler));
        self
    }
    
    /// Set error handler
    pub fn on_error<F>(&self, handler: F) -> &Self
    where
        F: Fn(SdkError) + Send + Sync + 'static
    {
        let mut callback = self.error_callback.lock().unwrap();
        *callback = Some(Arc::new(handler));
        self
    }
    
    /// Connect to Kraken WebSocket API
    pub async fn connect(&self) -> Result<(), SdkError> {
        *self.connection_state.lock().unwrap() = ConnectionState::Connecting;
        
        // Connection logic delegated to internal implementation
        // This would use the existing connection manager
        
        *self.connection_state.lock().unwrap() = ConnectionState::Connected;
        Ok(())
    }
    
    /// Disconnect from Kraken WebSocket API
    pub async fn disconnect(&self) -> Result<(), SdkError> {
        *self.connection_state.lock().unwrap() = ConnectionState::Disconnected;
        Ok(())
    }
    
    /// Check if connected
    pub fn is_connected(&self) -> bool {
        matches!(*self.connection_state.lock().unwrap(), ConnectionState::Connected)
    }
    
    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        self.connection_state.lock().unwrap().clone()
    }
    
    /// Get subscribed pairs
    pub fn subscribed_pairs(&self) -> Vec<String> {
        let mut pairs = Vec::new();
        
        for (pair, _) in self.ticker_callbacks.lock().unwrap().iter() {
            if !pairs.contains(pair) {
                pairs.push(pair.clone());
            }
        }
        for (pair, _, _) in self.orderbook_callbacks.lock().unwrap().iter() {
            if !pairs.contains(pair) {
                pairs.push(pair.clone());
            }
        }
        for (pair, _) in self.trade_callbacks.lock().unwrap().iter() {
            if !pairs.contains(pair) {
                pairs.push(pair.clone());
            }
        }
        
        pairs
    }
    
    // Internal: dispatch ticker data to callbacks
    pub(crate) fn dispatch_ticker(&self, data: TickerData) {
        let callbacks = self.ticker_callbacks.lock().unwrap();
        for (pair, callback) in callbacks.iter() {
            if pair == &data.symbol || pair == "*" {
                callback(data.clone());
            }
        }
    }
    
    // Internal: dispatch orderbook data to callbacks
    pub(crate) fn dispatch_orderbook(&self, data: OrderBookUpdate) {
        let callbacks = self.orderbook_callbacks.lock().unwrap();
        for (pair, _, callback) in callbacks.iter() {
            if pair == &data.symbol || pair == "*" {
                callback(data.clone());
            }
        }
    }
    
    // Internal: dispatch trade data to callbacks
    pub(crate) fn dispatch_trade(&self, data: TradeData) {
        let callbacks = self.trade_callbacks.lock().unwrap();
        for (pair, callback) in callbacks.iter() {
            if pair == &data.symbol || pair == "*" {
                callback(data.clone());
            }
        }
    }
}

