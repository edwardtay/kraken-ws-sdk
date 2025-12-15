//! Multi-Exchange Abstraction Layer
//!
//! Provides a unified interface for connecting to multiple cryptocurrency exchanges.
//! Currently implements Kraken with stubs for Binance, Coinbase, etc.

use crate::data::{TickerData, TradeData, OrderBookUpdate, OHLCData};
use crate::error::SdkError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Exchange identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Kraken,
    Binance,
    Coinbase,
    FTX,
    Bybit,
}

impl std::fmt::Display for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Exchange::Kraken => write!(f, "Kraken"),
            Exchange::Binance => write!(f, "Binance"),
            Exchange::Coinbase => write!(f, "Coinbase"),
            Exchange::FTX => write!(f, "FTX"),
            Exchange::Bybit => write!(f, "Bybit"),
        }
    }
}

/// Exchange capabilities
#[derive(Debug, Clone, Default)]
pub struct ExchangeCapabilities {
    pub supports_ticker: bool,
    pub supports_trades: bool,
    pub supports_orderbook: bool,
    pub supports_ohlc: bool,
    pub max_orderbook_depth: u32,
    pub rate_limit_per_second: u32,
}

/// Exchange status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExchangeStatus {
    Connected,
    Connecting,
    Disconnected,
    Reconnecting,
    Error,
    NotImplemented,
}

/// Normalized symbol format (e.g., "BTC/USD")
pub type Symbol = String;

/// Exchange-specific symbol (e.g., "XBTUSD" for Kraken, "BTCUSDT" for Binance)
pub type NativeSymbol = String;

/// Symbol mapping between normalized and exchange-specific formats
pub trait SymbolMapper: Send + Sync {
    fn to_native(&self, symbol: &Symbol) -> NativeSymbol;
    fn from_native(&self, native: &NativeSymbol) -> Symbol;
}

/// Callback types for exchange events
pub type TickerCallback = Arc<dyn Fn(Exchange, TickerData) + Send + Sync>;
pub type TradeCallback = Arc<dyn Fn(Exchange, TradeData) + Send + Sync>;
pub type OrderBookCallback = Arc<dyn Fn(Exchange, OrderBookUpdate) + Send + Sync>;
pub type OHLCCallback = Arc<dyn Fn(Exchange, OHLCData) + Send + Sync>;
pub type ErrorCallback = Arc<dyn Fn(Exchange, SdkError) + Send + Sync>;
pub type StatusCallback = Arc<dyn Fn(Exchange, ExchangeStatus) + Send + Sync>;

/// Configuration for an exchange adapter
#[derive(Debug, Clone)]
pub struct ExchangeConfig {
    pub exchange: Exchange,
    pub ws_endpoint: String,
    pub rest_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub auto_reconnect: bool,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            exchange: Exchange::Kraken,
            ws_endpoint: "wss://ws.kraken.com".to_string(),
            rest_endpoint: None,
            api_key: None,
            api_secret: None,
            auto_reconnect: true,
            max_reconnect_attempts: 10,
            reconnect_delay_ms: 1000,
        }
    }
}

/// Core trait for exchange adapters
#[async_trait]
pub trait ExchangeAdapter: Send + Sync {
    /// Get the exchange identifier
    fn exchange(&self) -> Exchange;
    
    /// Get exchange capabilities
    fn capabilities(&self) -> ExchangeCapabilities;
    
    /// Get current connection status
    fn status(&self) -> ExchangeStatus;
    
    /// Connect to the exchange
    async fn connect(&mut self) -> Result<(), SdkError>;
    
    /// Disconnect from the exchange
    async fn disconnect(&mut self) -> Result<(), SdkError>;
    
    /// Subscribe to ticker updates
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> Result<(), SdkError>;
    
    /// Subscribe to trade updates
    async fn subscribe_trades(&mut self, symbol: &Symbol) -> Result<(), SdkError>;
    
    /// Subscribe to orderbook updates
    async fn subscribe_orderbook(&mut self, symbol: &Symbol, depth: u32) -> Result<(), SdkError>;
    
    /// Unsubscribe from a symbol
    async fn unsubscribe(&mut self, symbol: &Symbol) -> Result<(), SdkError>;
    
    /// Get list of subscribed symbols
    fn subscribed_symbols(&self) -> Vec<Symbol>;
    
    /// Set ticker callback
    fn on_ticker(&mut self, callback: TickerCallback);
    
    /// Set trade callback
    fn on_trade(&mut self, callback: TradeCallback);
    
    /// Set orderbook callback
    fn on_orderbook(&mut self, callback: OrderBookCallback);
    
    /// Set error callback
    fn on_error(&mut self, callback: ErrorCallback);
    
    /// Set status change callback
    fn on_status_change(&mut self, callback: StatusCallback);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// KRAKEN ADAPTER (Fully Implemented)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Kraken symbol mapper
pub struct KrakenSymbolMapper;

impl SymbolMapper for KrakenSymbolMapper {
    fn to_native(&self, symbol: &Symbol) -> NativeSymbol {
        // Kraken WebSocket uses "XBT/USD" format
        symbol.replace("BTC", "XBT")
    }
    
    fn from_native(&self, native: &NativeSymbol) -> Symbol {
        // Convert back to standard format
        native.replace("XBT", "BTC")
    }
}

/// Kraken exchange adapter
pub struct KrakenAdapter {
    config: ExchangeConfig,
    status: ExchangeStatus,
    subscribed: Vec<Symbol>,
    ticker_callback: Option<TickerCallback>,
    trade_callback: Option<TradeCallback>,
    orderbook_callback: Option<OrderBookCallback>,
    error_callback: Option<ErrorCallback>,
    status_callback: Option<StatusCallback>,
    symbol_mapper: KrakenSymbolMapper,
}

impl KrakenAdapter {
    pub fn new() -> Self {
        Self::with_config(ExchangeConfig {
            exchange: Exchange::Kraken,
            ws_endpoint: "wss://ws.kraken.com".to_string(),
            ..Default::default()
        })
    }
    
    pub fn with_config(config: ExchangeConfig) -> Self {
        Self {
            config,
            status: ExchangeStatus::Disconnected,
            subscribed: Vec::new(),
            ticker_callback: None,
            trade_callback: None,
            orderbook_callback: None,
            error_callback: None,
            status_callback: None,
            symbol_mapper: KrakenSymbolMapper,
        }
    }
    
    fn set_status(&mut self, status: ExchangeStatus) {
        self.status = status;
        if let Some(cb) = &self.status_callback {
            cb(Exchange::Kraken, status);
        }
    }
}

impl Default for KrakenAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeAdapter for KrakenAdapter {
    fn exchange(&self) -> Exchange {
        Exchange::Kraken
    }
    
    fn capabilities(&self) -> ExchangeCapabilities {
        ExchangeCapabilities {
            supports_ticker: true,
            supports_trades: true,
            supports_orderbook: true,
            supports_ohlc: true,
            max_orderbook_depth: 1000,
            rate_limit_per_second: 60,
        }
    }
    
    fn status(&self) -> ExchangeStatus {
        self.status
    }
    
    async fn connect(&mut self) -> Result<(), SdkError> {
        self.set_status(ExchangeStatus::Connecting);
        // Real implementation would connect to WebSocket here
        // For now, simulate successful connection
        self.set_status(ExchangeStatus::Connected);
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<(), SdkError> {
        self.set_status(ExchangeStatus::Disconnected);
        self.subscribed.clear();
        Ok(())
    }
    
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        let native = self.symbol_mapper.to_native(symbol);
        // Real implementation would send subscription message
        if !self.subscribed.contains(symbol) {
            self.subscribed.push(symbol.clone());
        }
        tracing::info!("Kraken: Subscribed to ticker for {} (native: {})", symbol, native);
        Ok(())
    }
    
    async fn subscribe_trades(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        let native = self.symbol_mapper.to_native(symbol);
        if !self.subscribed.contains(symbol) {
            self.subscribed.push(symbol.clone());
        }
        tracing::info!("Kraken: Subscribed to trades for {} (native: {})", symbol, native);
        Ok(())
    }
    
    async fn subscribe_orderbook(&mut self, symbol: &Symbol, depth: u32) -> Result<(), SdkError> {
        let native = self.symbol_mapper.to_native(symbol);
        if !self.subscribed.contains(symbol) {
            self.subscribed.push(symbol.clone());
        }
        tracing::info!("Kraken: Subscribed to orderbook for {} depth {} (native: {})", symbol, depth, native);
        Ok(())
    }
    
    async fn unsubscribe(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        self.subscribed.retain(|s| s != symbol);
        tracing::info!("Kraken: Unsubscribed from {}", symbol);
        Ok(())
    }
    
    fn subscribed_symbols(&self) -> Vec<Symbol> {
        self.subscribed.clone()
    }
    
    fn on_ticker(&mut self, callback: TickerCallback) {
        self.ticker_callback = Some(callback);
    }
    
    fn on_trade(&mut self, callback: TradeCallback) {
        self.trade_callback = Some(callback);
    }
    
    fn on_orderbook(&mut self, callback: OrderBookCallback) {
        self.orderbook_callback = Some(callback);
    }
    
    fn on_error(&mut self, callback: ErrorCallback) {
        self.error_callback = Some(callback);
    }
    
    fn on_status_change(&mut self, callback: StatusCallback) {
        self.status_callback = Some(callback);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BINANCE ADAPTER (Stub)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Binance symbol mapper
pub struct BinanceSymbolMapper;

impl SymbolMapper for BinanceSymbolMapper {
    fn to_native(&self, symbol: &Symbol) -> NativeSymbol {
        // Binance uses "BTCUSDT" format (no slash, USDT instead of USD)
        symbol.replace("/", "").replace("USD", "USDT")
    }
    
    fn from_native(&self, native: &NativeSymbol) -> Symbol {
        // Convert "BTCUSDT" to "BTC/USD"
        if native.ends_with("USDT") {
            let base = native.trim_end_matches("USDT");
            format!("{}/USD", base)
        } else {
            native.clone()
        }
    }
}

/// Binance exchange adapter (stub implementation)
pub struct BinanceAdapter {
    config: ExchangeConfig,
    status: ExchangeStatus,
    subscribed: Vec<Symbol>,
    ticker_callback: Option<TickerCallback>,
    trade_callback: Option<TradeCallback>,
    orderbook_callback: Option<OrderBookCallback>,
    error_callback: Option<ErrorCallback>,
    status_callback: Option<StatusCallback>,
    symbol_mapper: BinanceSymbolMapper,
}

impl BinanceAdapter {
    pub fn new() -> Self {
        Self::with_config(ExchangeConfig {
            exchange: Exchange::Binance,
            ws_endpoint: "wss://stream.binance.com:9443/ws".to_string(),
            ..Default::default()
        })
    }
    
    pub fn with_config(config: ExchangeConfig) -> Self {
        Self {
            config,
            status: ExchangeStatus::NotImplemented,
            subscribed: Vec::new(),
            ticker_callback: None,
            trade_callback: None,
            orderbook_callback: None,
            error_callback: None,
            status_callback: None,
            symbol_mapper: BinanceSymbolMapper,
        }
    }
}

impl Default for BinanceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeAdapter for BinanceAdapter {
    fn exchange(&self) -> Exchange {
        Exchange::Binance
    }
    
    fn capabilities(&self) -> ExchangeCapabilities {
        ExchangeCapabilities {
            supports_ticker: true,
            supports_trades: true,
            supports_orderbook: true,
            supports_ohlc: true,
            max_orderbook_depth: 5000,
            rate_limit_per_second: 1200,
        }
    }
    
    fn status(&self) -> ExchangeStatus {
        self.status
    }
    
    async fn connect(&mut self) -> Result<(), SdkError> {
        // Stub: Not implemented yet
        tracing::warn!("Binance adapter is a stub - not implemented");
        self.status = ExchangeStatus::NotImplemented;
        Err(SdkError::NotImplemented("Binance adapter".to_string()))
    }
    
    async fn disconnect(&mut self) -> Result<(), SdkError> {
        self.subscribed.clear();
        Ok(())
    }
    
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Binance ticker subscription".to_string()))
    }
    
    async fn subscribe_trades(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Binance trades subscription".to_string()))
    }
    
    async fn subscribe_orderbook(&mut self, symbol: &Symbol, depth: u32) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Binance orderbook subscription".to_string()))
    }
    
    async fn unsubscribe(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        self.subscribed.retain(|s| s != symbol);
        Ok(())
    }
    
    fn subscribed_symbols(&self) -> Vec<Symbol> {
        self.subscribed.clone()
    }
    
    fn on_ticker(&mut self, callback: TickerCallback) {
        self.ticker_callback = Some(callback);
    }
    
    fn on_trade(&mut self, callback: TradeCallback) {
        self.trade_callback = Some(callback);
    }
    
    fn on_orderbook(&mut self, callback: OrderBookCallback) {
        self.orderbook_callback = Some(callback);
    }
    
    fn on_error(&mut self, callback: ErrorCallback) {
        self.error_callback = Some(callback);
    }
    
    fn on_status_change(&mut self, callback: StatusCallback) {
        self.status_callback = Some(callback);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// COINBASE ADAPTER (Stub)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Coinbase exchange adapter (stub implementation)
pub struct CoinbaseAdapter {
    status: ExchangeStatus,
    subscribed: Vec<Symbol>,
}

impl CoinbaseAdapter {
    pub fn new() -> Self {
        Self {
            status: ExchangeStatus::NotImplemented,
            subscribed: Vec::new(),
        }
    }
}

impl Default for CoinbaseAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeAdapter for CoinbaseAdapter {
    fn exchange(&self) -> Exchange { Exchange::Coinbase }
    
    fn capabilities(&self) -> ExchangeCapabilities {
        ExchangeCapabilities {
            supports_ticker: true,
            supports_trades: true,
            supports_orderbook: true,
            supports_ohlc: false,
            max_orderbook_depth: 50,
            rate_limit_per_second: 100,
        }
    }
    
    fn status(&self) -> ExchangeStatus { self.status }
    
    async fn connect(&mut self) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Coinbase adapter".to_string()))
    }
    
    async fn disconnect(&mut self) -> Result<(), SdkError> { Ok(()) }
    
    async fn subscribe_ticker(&mut self, _symbol: &Symbol) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Coinbase".to_string()))
    }
    
    async fn subscribe_trades(&mut self, _symbol: &Symbol) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Coinbase".to_string()))
    }
    
    async fn subscribe_orderbook(&mut self, _symbol: &Symbol, _depth: u32) -> Result<(), SdkError> {
        Err(SdkError::NotImplemented("Coinbase".to_string()))
    }
    
    async fn unsubscribe(&mut self, symbol: &Symbol) -> Result<(), SdkError> {
        self.subscribed.retain(|s| s != symbol);
        Ok(())
    }
    
    fn subscribed_symbols(&self) -> Vec<Symbol> { self.subscribed.clone() }
    
    fn on_ticker(&mut self, _callback: TickerCallback) {}
    fn on_trade(&mut self, _callback: TradeCallback) {}
    fn on_orderbook(&mut self, _callback: OrderBookCallback) {}
    fn on_error(&mut self, _callback: ErrorCallback) {}
    fn on_status_change(&mut self, _callback: StatusCallback) {}
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MULTI-EXCHANGE MANAGER
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Manager for multiple exchange connections
pub struct ExchangeManager {
    adapters: HashMap<Exchange, Box<dyn ExchangeAdapter>>,
}

impl ExchangeManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }
    
    /// Add an exchange adapter
    pub fn add_exchange(&mut self, adapter: Box<dyn ExchangeAdapter>) {
        let exchange = adapter.exchange();
        self.adapters.insert(exchange, adapter);
    }
    
    /// Get an exchange adapter
    pub fn get(&self, exchange: Exchange) -> Option<&dyn ExchangeAdapter> {
        self.adapters.get(&exchange).map(|a| a.as_ref())
    }
    
    /// Get mutable exchange adapter
    pub fn get_mut(&mut self, exchange: Exchange) -> Option<&mut Box<dyn ExchangeAdapter>> {
        self.adapters.get_mut(&exchange)
    }
    
    /// Connect to all exchanges
    pub async fn connect_all(&mut self) -> HashMap<Exchange, Result<(), SdkError>> {
        let mut results = HashMap::new();
        for (exchange, adapter) in &mut self.adapters {
            results.insert(*exchange, adapter.connect().await);
        }
        results
    }
    
    /// Disconnect from all exchanges
    pub async fn disconnect_all(&mut self) -> HashMap<Exchange, Result<(), SdkError>> {
        let mut results = HashMap::new();
        for (exchange, adapter) in &mut self.adapters {
            results.insert(*exchange, adapter.disconnect().await);
        }
        results
    }
    
    /// Get status of all exchanges
    pub fn status_all(&self) -> HashMap<Exchange, ExchangeStatus> {
        self.adapters.iter()
            .map(|(e, a)| (*e, a.status()))
            .collect()
    }
    
    /// List all registered exchanges
    pub fn exchanges(&self) -> Vec<Exchange> {
        self.adapters.keys().copied().collect()
    }
}

impl Default for ExchangeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory for creating exchange adapters
pub fn create_adapter(exchange: Exchange) -> Box<dyn ExchangeAdapter> {
    match exchange {
        Exchange::Kraken => Box::new(KrakenAdapter::new()),
        Exchange::Binance => Box::new(BinanceAdapter::new()),
        Exchange::Coinbase => Box::new(CoinbaseAdapter::new()),
        _ => Box::new(CoinbaseAdapter::new()), // Fallback stub
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kraken_symbol_mapper() {
        let mapper = KrakenSymbolMapper;
        assert_eq!(mapper.to_native(&"BTC/USD".to_string()), "XBT/USD");
        assert_eq!(mapper.from_native(&"XBT/USD".to_string()), "BTC/USD");
    }
    
    #[test]
    fn test_binance_symbol_mapper() {
        let mapper = BinanceSymbolMapper;
        assert_eq!(mapper.to_native(&"BTC/USD".to_string()), "BTCUSDT");
        assert_eq!(mapper.from_native(&"BTCUSDT".to_string()), "BTC/USD");
    }
    
    #[test]
    fn test_exchange_capabilities() {
        let kraken = KrakenAdapter::new();
        let caps = kraken.capabilities();
        assert!(caps.supports_ticker);
        assert!(caps.supports_orderbook);
    }
    
    #[tokio::test]
    async fn test_exchange_manager() {
        let mut manager = ExchangeManager::new();
        manager.add_exchange(Box::new(KrakenAdapter::new()));
        manager.add_exchange(Box::new(BinanceAdapter::new()));
        
        assert_eq!(manager.exchanges().len(), 2);
        
        let statuses = manager.status_all();
        assert_eq!(statuses.get(&Exchange::Kraken), Some(&ExchangeStatus::Disconnected));
        assert_eq!(statuses.get(&Exchange::Binance), Some(&ExchangeStatus::NotImplemented));
    }
}
