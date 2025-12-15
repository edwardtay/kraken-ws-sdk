//! Integration tests for the Kraken WebSocket SDK

use kraken_ws_sdk::{
    init_logging, Channel, ClientConfig, DataType, EventCallback, KrakenWsClient,
    TickerData, TradeData, OrderBookUpdate, OHLCData, ConnectionState, SdkError,
    ReconnectConfig,
};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Duration;
use tokio_test;

/// Test callback for integration tests
struct TestCallback {
    ticker_count: AtomicU64,
    trade_count: AtomicU64,
    orderbook_count: AtomicU64,
    error_count: AtomicU64,
}

impl TestCallback {
    fn new() -> Self {
        Self {
            ticker_count: AtomicU64::new(0),
            trade_count: AtomicU64::new(0),
            orderbook_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }
    
    fn get_ticker_count(&self) -> u64 {
        self.ticker_count.load(Ordering::Relaxed)
    }
    
    fn get_trade_count(&self) -> u64 {
        self.trade_count.load(Ordering::Relaxed)
    }
    
    fn get_orderbook_count(&self) -> u64 {
        self.orderbook_count.load(Ordering::Relaxed)
    }
    
    fn get_error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }
}

impl EventCallback for TestCallback {
    fn on_ticker(&self, _data: TickerData) {
        self.ticker_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_orderbook(&self, _data: OrderBookUpdate) {
        self.orderbook_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_trade(&self, _data: TradeData) {
        self.trade_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_ohlc(&self, _data: OHLCData) {
        // Not counting OHLC for these tests
    }
    
    fn on_error(&self, _error: SdkError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_connection_state_change(&self, _state: ConnectionState) {
        // Not counting connection state changes for these tests
    }
}

#[tokio::test]
async fn test_client_creation() {
    let config = ClientConfig::default();
    let client = KrakenWsClient::new(config);
    
    assert!(!client.is_connected());
    assert_eq!(client.connection_state(), ConnectionState::Disconnected);
}

#[tokio::test]
async fn test_callback_registration() {
    let config = ClientConfig::default();
    let client = KrakenWsClient::new(config);
    
    let callback: Arc<dyn EventCallback> = Arc::new(TestCallback::new());
    
    let ticker_id = client.register_callback(DataType::Ticker, callback.clone());
    let trade_id = client.register_callback(DataType::Trade, callback.clone());
    
    assert!(ticker_id > 0);
    assert!(trade_id > 0);
    assert_ne!(ticker_id, trade_id);
    
    assert_eq!(client.get_callback_count(&DataType::Ticker), 1);
    assert_eq!(client.get_callback_count(&DataType::Trade), 1);
    assert_eq!(client.get_callback_count(&DataType::OrderBook), 0);
}

#[tokio::test]
async fn test_subscription_creation() {
    let config = ClientConfig::default();
    let mut client = KrakenWsClient::new(config);
    
    let channels = vec![
        Channel::new("ticker").with_symbol("BTC/USD"),
        Channel::new("trade").with_symbol("BTC/USD"),
    ];
    
    // This should not fail even without connection (creates subscription message)
    let result = client.subscribe(channels).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_config_validation() {
    // Test valid config
    let valid_config = ClientConfig {
        endpoint: "wss://ws.kraken.com".to_string(),
        timeout: Duration::from_secs(30),
        buffer_size: 1024,
        ..Default::default()
    };
    assert!(valid_config.validate().is_ok());
    
    // Test invalid endpoint
    let invalid_config = ClientConfig {
        endpoint: "invalid-url".to_string(),
        ..Default::default()
    };
    assert!(invalid_config.validate().is_err());
    
    // Test zero buffer size
    let zero_buffer_config = ClientConfig {
        buffer_size: 0,
        ..Default::default()
    };
    assert!(zero_buffer_config.validate().is_err());
}

#[tokio::test]
async fn test_reconnect_config_validation() {
    // Test valid reconnect config
    let valid_config = ReconnectConfig {
        max_attempts: 5,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
        backoff_multiplier: 2.0,
    };
    assert!(valid_config.validate().is_ok());
    
    // Test invalid backoff multiplier
    let invalid_config = ReconnectConfig {
        backoff_multiplier: 0.5, // Should be > 1.0
        ..Default::default()
    };
    assert!(invalid_config.validate().is_err());
    
    // Test max delay less than initial delay
    let invalid_delay_config = ReconnectConfig {
        initial_delay: Duration::from_secs(60),
        max_delay: Duration::from_secs(30), // Less than initial
        ..Default::default()
    };
    assert!(invalid_delay_config.validate().is_err());
}

#[tokio::test]
async fn test_client_builder() {
    use kraken_ws_sdk::ClientConfigBuilder;
    
    let config = ClientConfigBuilder::new()
        .endpoint("wss://test.example.com")
        .api_credentials("test_key", "test_secret")
        .buffer_size(2048)
        .timeout(Duration::from_secs(45))
        .build();
    
    assert_eq!(config.endpoint, "wss://test.example.com");
    assert_eq!(config.api_key, Some("test_key".to_string()));
    assert_eq!(config.api_secret, Some("test_secret".to_string()));
    assert_eq!(config.buffer_size, 2048);
    assert_eq!(config.timeout, Duration::from_secs(45));
}

#[tokio::test]
async fn test_channel_creation() {
    let channel1 = Channel::new("ticker");
    assert_eq!(channel1.name, "ticker");
    assert_eq!(channel1.symbol, None);
    assert_eq!(channel1.interval, None);
    
    let channel2 = Channel::new("ohlc")
        .with_symbol("BTC/USD")
        .with_interval("5");
    assert_eq!(channel2.name, "ohlc");
    assert_eq!(channel2.symbol, Some("BTC/USD".to_string()));
    assert_eq!(channel2.interval, Some("5".to_string()));
}

#[tokio::test]
async fn test_multiple_callbacks() {
    let config = ClientConfig::default();
    let client = KrakenWsClient::new(config);
    
    let callback1: Arc<dyn EventCallback> = Arc::new(TestCallback::new());
    let callback2: Arc<dyn EventCallback> = Arc::new(TestCallback::new());
    
    // Register multiple callbacks for the same data type
    let id1 = client.register_callback(DataType::Ticker, callback1);
    let id2 = client.register_callback(DataType::Ticker, callback2);
    
    assert_ne!(id1, id2);
    assert_eq!(client.get_callback_count(&DataType::Ticker), 2);
}

#[tokio::test]
async fn test_cleanup() {
    let config = ClientConfig::default();
    let mut client = KrakenWsClient::new(config);
    
    // Register some callbacks
    let callback: Arc<dyn EventCallback> = Arc::new(TestCallback::new());
    client.register_callback(DataType::Ticker, callback);
    
    // Cleanup should not fail
    let result = client.cleanup().await;
    assert!(result.is_ok());
}

// Mock WebSocket server for testing (would be implemented with a test framework)
// This is a placeholder for more comprehensive integration tests
#[tokio::test]
#[ignore] // Ignore by default as it requires a mock server
async fn test_real_connection() {
    // This test would require setting up a mock WebSocket server
    // that simulates Kraken's WebSocket API responses
    
    let config = ClientConfig {
        endpoint: "ws://localhost:8080".to_string(), // Mock server
        timeout: Duration::from_secs(5),
        ..Default::default()
    };
    
    let mut client = KrakenWsClient::new(config);
    let callback: Arc<dyn EventCallback> = Arc::new(TestCallback::new());
    
    client.register_callback(DataType::Ticker, callback.clone());
    
    // This would fail without a mock server running
    // let result = client.connect().await;
    // assert!(result.is_ok());
}