//! Unit tests for individual modules

use kraken_ws_sdk::{
    data::*,
    error::*,
    events::*,
    orderbook::*,
    parser::*,
    subscription::*,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::str::FromStr;
use chrono::Utc;

// Test EventDispatcher
#[tokio::test]
async fn test_event_dispatcher_creation() {
    let dispatcher = EventDispatcher::new();
    assert_eq!(dispatcher.get_callback_count(&DataType::Ticker), 0);
    assert_eq!(dispatcher.get_connection_listener_count(), 0);
}

#[tokio::test]
async fn test_event_dispatcher_callback_registration() {
    let dispatcher = EventDispatcher::new();
    
    struct TestCallback;
    impl EventCallback for TestCallback {
        fn on_ticker(&self, _data: TickerData) {}
        fn on_orderbook(&self, _data: OrderBookUpdate) {}
        fn on_trade(&self, _data: TradeData) {}
        fn on_ohlc(&self, _data: OHLCData) {}
        fn on_error(&self, _error: SdkError) {}
        fn on_connection_state_change(&self, _state: ConnectionState) {}
    }
    
    let callback: Arc<dyn EventCallback> = Arc::new(TestCallback);
    
    let id1 = dispatcher.register_callback(DataType::Ticker, callback.clone());
    let id2 = dispatcher.register_callback(DataType::Trade, callback.clone());
    
    assert!(id1 > 0);
    assert!(id2 > 0);
    assert_ne!(id1, id2);
    
    assert_eq!(dispatcher.get_callback_count(&DataType::Ticker), 1);
    assert_eq!(dispatcher.get_callback_count(&DataType::Trade), 1);
    assert_eq!(dispatcher.get_callback_count(&DataType::OrderBook), 0);
}

#[tokio::test]
async fn test_event_dispatcher_unregister() {
    let dispatcher = EventDispatcher::new();
    
    struct TestCallback;
    impl EventCallback for TestCallback {
        fn on_ticker(&self, _data: TickerData) {}
        fn on_orderbook(&self, _data: OrderBookUpdate) {}
        fn on_trade(&self, _data: TradeData) {}
        fn on_ohlc(&self, _data: OHLCData) {}
        fn on_error(&self, _error: SdkError) {}
        fn on_connection_state_change(&self, _state: ConnectionState) {}
    }
    
    let callback: Arc<dyn EventCallback> = Arc::new(TestCallback);
    let id = dispatcher.register_callback(DataType::Ticker, callback);
    
    assert_eq!(dispatcher.get_callback_count(&DataType::Ticker), 1);
    
    let removed = dispatcher.unregister_callback(DataType::Ticker, id);
    assert!(removed);
    assert_eq!(dispatcher.get_callback_count(&DataType::Ticker), 0);
    
    // Try to remove again - should return false
    let removed_again = dispatcher.unregister_callback(DataType::Ticker, id);
    assert!(!removed_again);
}

// Test OrderBookManager
#[tokio::test]
async fn test_orderbook_manager_creation() {
    let manager = OrderBookManager::new();
    assert_eq!(manager.get_symbols().len(), 0);
    assert!(manager.get_order_book("BTC/USD").is_none());
}

#[tokio::test]
async fn test_orderbook_manager_update() {
    let manager = OrderBookManager::new();
    
    let update = OrderBookUpdate {
        symbol: "BTC/USD".to_string(),
        bids: vec![
            PriceLevel {
                price: Decimal::from_str("50000.0").unwrap(),
                volume: Decimal::from_str("1.5").unwrap(),
                timestamp: Utc::now(),
            },
            PriceLevel {
                price: Decimal::from_str("49999.0").unwrap(),
                volume: Decimal::from_str("2.0").unwrap(),
                timestamp: Utc::now(),
            },
        ],
        asks: vec![
            PriceLevel {
                price: Decimal::from_str("50001.0").unwrap(),
                volume: Decimal::from_str("1.0").unwrap(),
                timestamp: Utc::now(),
            },
            PriceLevel {
                price: Decimal::from_str("50002.0").unwrap(),
                volume: Decimal::from_str("0.5").unwrap(),
                timestamp: Utc::now(),
            },
        ],
        timestamp: Utc::now(),
        checksum: None,
    };
    
    let result = manager.apply_update(update);
    assert!(result.is_ok());
    
    let order_book = manager.get_order_book("BTC/USD");
    assert!(order_book.is_some());
    
    let ob = order_book.unwrap();
    assert_eq!(ob.bids.len(), 2);
    assert_eq!(ob.asks.len(), 2);
    
    // Test best bid/ask
    let (best_bid, best_ask) = manager.get_best_bid_ask("BTC/USD").unwrap();
    assert_eq!(best_bid, Some(Decimal::from_str("50000.0").unwrap()));
    assert_eq!(best_ask, Some(Decimal::from_str("50001.0").unwrap()));
}

#[tokio::test]
async fn test_orderbook_spread_calculation() {
    let mut order_book = OrderBook::new("BTC/USD");
    
    // Add some price levels
    order_book.bids.insert(
        Decimal::from_str("50000.0").unwrap(),
        PriceLevel {
            price: Decimal::from_str("50000.0").unwrap(),
            volume: Decimal::from_str("1.0").unwrap(),
            timestamp: Utc::now(),
        },
    );
    
    order_book.asks.insert(
        Decimal::from_str("50001.0").unwrap(),
        PriceLevel {
            price: Decimal::from_str("50001.0").unwrap(),
            volume: Decimal::from_str("1.0").unwrap(),
            timestamp: Utc::now(),
        },
    );
    
    let spread = order_book.get_spread();
    assert_eq!(spread, Some(Decimal::from_str("1.0").unwrap()));
    
    let mid_price = order_book.get_mid_price();
    assert_eq!(mid_price, Some(Decimal::from_str("50000.5").unwrap()));
}

// Test SubscriptionManager
#[tokio::test]
async fn test_subscription_manager_creation() {
    let manager = SubscriptionManager::new();
    assert_eq!(manager.get_active_subscriptions().len(), 0);
}

#[tokio::test]
async fn test_subscription_message_creation() {
    let manager = SubscriptionManager::new();
    
    let channels = vec![
        Channel::new("ticker").with_symbol("BTC/USD"),
    ];
    
    let result = manager.create_subscription_message(&channels);
    assert!(result.is_ok());
    
    let message = result.unwrap();
    if let tokio_tungstenite::tungstenite::Message::Text(text) = message {
        assert!(text.contains("subscribe"));
        assert!(text.contains("ticker"));
        assert!(text.contains("BTC/USD"));
    } else {
        panic!("Expected text message");
    }
}

#[tokio::test]
async fn test_subscription_validation() {
    let manager = SubscriptionManager::new();
    
    // Valid channel
    let valid_channels = vec![Channel::new("ticker").with_symbol("BTC/USD")];
    let result = manager.create_subscription_message(&valid_channels);
    assert!(result.is_ok());
    
    // Invalid channel name
    let invalid_channels = vec![Channel::new("invalid_channel")];
    let result = manager.create_subscription_message(&invalid_channels);
    assert!(result.is_err());
    
    // Invalid OHLC interval
    let invalid_ohlc = vec![
        Channel::new("ohlc")
            .with_symbol("BTC/USD")
            .with_interval("invalid")
    ];
    let result = manager.create_subscription_message(&invalid_ohlc);
    assert!(result.is_err());
}

// Test KrakenDataParser
#[tokio::test]
async fn test_kraken_parser_creation() {
    let parser = KrakenDataParser::new();
    // Parser should be created successfully
    // We can't test much without actual data, but creation should work
}

#[tokio::test]
async fn test_kraken_parser_invalid_json() {
    let parser = KrakenDataParser::new();
    
    let invalid_json = "invalid json data";
    
    let ticker_result = parser.parse_ticker(invalid_json);
    assert!(ticker_result.is_err());
    
    let trade_result = parser.parse_trade(invalid_json);
    assert!(trade_result.is_err());
    
    let orderbook_result = parser.parse_orderbook(invalid_json);
    assert!(orderbook_result.is_err());
    
    let ohlc_result = parser.parse_ohlc(invalid_json);
    assert!(ohlc_result.is_err());
}

#[tokio::test]
async fn test_kraken_parser_empty_data() {
    let parser = KrakenDataParser::new();
    
    let empty_json = "{}";
    
    let ticker_result = parser.parse_ticker(empty_json);
    assert!(ticker_result.is_err());
    
    let trade_result = parser.parse_trade(empty_json);
    assert!(trade_result.is_err());
}

// Test MessageHandler
#[tokio::test]
async fn test_message_handler_creation() {
    let parser: Arc<dyn DataParser> = Arc::new(KrakenDataParser::new());
    let dispatcher = Arc::new(EventDispatcher::new());
    
    let handler = MessageHandler::new(parser, dispatcher);
    // Handler should be created successfully
}

#[tokio::test]
async fn test_message_handler_empty_message() {
    let parser: Arc<dyn DataParser> = Arc::new(KrakenDataParser::new());
    let dispatcher = Arc::new(EventDispatcher::new());
    let handler = MessageHandler::new(parser, dispatcher);
    
    let result = handler.handle_message("").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_message_handler_system_messages() {
    let parser: Arc<dyn DataParser> = Arc::new(KrakenDataParser::new());
    let dispatcher = Arc::new(EventDispatcher::new());
    let handler = MessageHandler::new(parser, dispatcher);
    
    // Test subscription status message
    let subscription_msg = r#"{"event":"subscriptionStatus","status":"subscribed"}"#;
    let result = handler.handle_message(subscription_msg).await;
    assert!(result.is_ok());
    
    // Test system status message
    let system_msg = r#"{"event":"systemStatus","status":"online"}"#;
    let result = handler.handle_message(system_msg).await;
    assert!(result.is_ok());
    
    // Test heartbeat message
    let heartbeat_msg = r#"{"event":"heartbeat"}"#;
    let result = handler.handle_message(heartbeat_msg).await;
    assert!(result.is_ok());
}

// Test data structures
#[tokio::test]
async fn test_ticker_data_display() {
    let ticker = TickerData {
        symbol: "BTC/USD".to_string(),
        bid: Decimal::from_str("50000.0").unwrap(),
        ask: Decimal::from_str("50001.0").unwrap(),
        last_price: Decimal::from_str("50000.5").unwrap(),
        volume: Decimal::from_str("100.0").unwrap(),
        timestamp: Utc::now(),
    };
    
    let display_str = format!("{}", ticker);
    assert!(display_str.contains("BTC/USD"));
    assert!(display_str.contains("50000"));
}

#[tokio::test]
async fn test_trade_data_display() {
    let trade = TradeData {
        symbol: "BTC/USD".to_string(),
        price: Decimal::from_str("50000.0").unwrap(),
        volume: Decimal::from_str("1.5").unwrap(),
        side: TradeSide::Buy,
        timestamp: Utc::now(),
        trade_id: "test-123".to_string(),
    };
    
    let display_str = format!("{}", trade);
    assert!(display_str.contains("BTC/USD"));
    assert!(display_str.contains("Buy"));
    assert!(display_str.contains("test-123"));
}

#[tokio::test]
async fn test_price_level_display() {
    let level = PriceLevel {
        price: Decimal::from_str("50000.0").unwrap(),
        volume: Decimal::from_str("1.5").unwrap(),
        timestamp: Utc::now(),
    };
    
    let display_str = format!("{}", level);
    assert!(display_str.contains("1.5@50000"));
}

// Test error types
#[tokio::test]
async fn test_error_severity() {
    let config_error = SdkError::Configuration("test error".to_string());
    let severity = ErrorSeverity::from_error(&config_error);
    assert_eq!(severity, ErrorSeverity::High);
    
    let parse_error = SdkError::Parse(ParseError::InvalidJson("test".to_string()));
    let severity = ErrorSeverity::from_error(&parse_error);
    assert_eq!(severity, ErrorSeverity::Low);
    
    let auth_error = SdkError::Authentication("test".to_string());
    let severity = ErrorSeverity::from_error(&auth_error);
    assert_eq!(severity, ErrorSeverity::High);
}

#[tokio::test]
async fn test_error_context() {
    let context = ErrorContext::new("test_operation")
        .with_detail("key1", "value1")
        .with_detail("key2", "value2");
    
    assert_eq!(context.operation, "test_operation");
    assert_eq!(context.details.get("key1"), Some(&"value1".to_string()));
    assert_eq!(context.details.get("key2"), Some(&"value2".to_string()));
    
    let display_str = format!("{}", context);
    assert!(display_str.contains("test_operation"));
}

#[tokio::test]
async fn test_contextual_error() {
    let error = SdkError::Configuration("test error".to_string());
    let context = ErrorContext::new("test_operation");
    let contextual_error = ContextualError::new(error, context);
    
    let display_str = format!("{}", contextual_error);
    assert!(display_str.contains("test error"));
    assert!(display_str.contains("test_operation"));
}