//! Basic usage example for the Kraken WebSocket SDK

use kraken_ws_sdk::{
    init_logging, Channel, ClientConfig, DataType, EventCallback, KrakenWsClient, 
    TickerData, TradeData, OrderBookUpdate, OHLCData, ConnectionState, SdkError
};
use std::sync::Arc;
use std::time::Duration;

/// Example callback implementation
struct ExampleCallback;

impl EventCallback for ExampleCallback {
    fn on_ticker(&self, data: TickerData) {
        println!("📊 Ticker: {}", data);
    }
    
    fn on_orderbook(&self, data: OrderBookUpdate) {
        println!("📖 Order Book: {}", data);
    }
    
    fn on_trade(&self, data: TradeData) {
        println!("💰 Trade: {}", data);
    }
    
    fn on_ohlc(&self, data: OHLCData) {
        println!("📈 OHLC: {}", data);
    }
    
    fn on_error(&self, error: SdkError) {
        eprintln!("❌ Error: {}", error);
    }
    
    fn on_connection_state_change(&self, state: ConnectionState) {
        println!("🔗 Connection state: {:?}", state);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging();
    
    println!("🦑 Kraken WebSocket SDK - Basic Usage Example");
    println!("📝 Note: This example demonstrates the SDK API without connecting to real WebSocket");
    
    // Create client configuration
    let config = ClientConfig {
        endpoint: "wss://ws.kraken.com".to_string(),
        timeout: Duration::from_secs(30),
        buffer_size: 1024,
        ..Default::default()
    };
    
    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("❌ Configuration validation failed: {}", e);
        return Ok(());
    }
    println!("✅ Configuration validated successfully");
    
    // Create the WebSocket client
    let mut client = KrakenWsClient::new(config);
    
    // Create callback instance
    let callback: Arc<dyn EventCallback> = Arc::new(ExampleCallback);
    
    // Register callbacks for different data types
    let ticker_id = client.register_callback(DataType::Ticker, callback.clone());
    let trade_id = client.register_callback(DataType::Trade, callback.clone());
    let orderbook_id = client.register_callback(DataType::OrderBook, callback.clone());
    
    // Register connection state listener
    let connection_id = client.register_connection_listener(callback.clone());
    
    println!("✅ Registered callbacks:");
    println!("   Ticker: {}", ticker_id);
    println!("   Trade: {}", trade_id);
    println!("   OrderBook: {}", orderbook_id);
    println!("   Connection: {}", connection_id);
    
    // Verify callback counts
    println!("📊 Callback counts:");
    println!("   Ticker callbacks: {}", client.get_callback_count(&DataType::Ticker));
    println!("   Trade callbacks: {}", client.get_callback_count(&DataType::Trade));
    println!("   OrderBook callbacks: {}", client.get_callback_count(&DataType::OrderBook));
    
    // Create subscription channels
    let channels = vec![
        Channel::new("ticker").with_symbol("BTC/USD"),
        Channel::new("trade").with_symbol("BTC/USD"),
        Channel::new("book").with_symbol("BTC/USD"),
        Channel::new("ohlc").with_symbol("BTC/USD").with_interval("1"),
    ];
    
    println!("📡 Creating subscription messages for {} channels...", channels.len());
    
    // Test subscription message creation (without actual connection)
    if let Err(e) = client.subscribe(channels).await {
        eprintln!("❌ Failed to create subscription: {}", e);
        return Ok(());
    }
    println!("✅ Subscription messages created successfully");
    
    // Display current state
    println!("🔗 Connection state: {:?}", client.connection_state());
    println!("📊 Active subscriptions: {:?}", client.get_active_subscriptions());
    
    // Test channel validation
    println!("🔍 Testing channel validation...");
    
    // Valid channels
    let valid_channels = vec![
        Channel::new("ticker").with_symbol("ETH/USD"),
        Channel::new("ohlc").with_symbol("ETH/USD").with_interval("5"),
    ];
    
    match client.subscribe(valid_channels).await {
        Ok(_) => println!("✅ Valid channels accepted"),
        Err(e) => println!("❌ Unexpected error with valid channels: {}", e),
    }
    
    // Test order book functionality (simulated)
    println!("📖 Testing order book functionality...");
    
    // Since we're not connected, order book will be empty
    match client.get_order_book("BTC/USD") {
        Some(order_book) => {
            println!("📖 Order book found for BTC/USD");
            if let Some(spread) = order_book.get_spread() {
                println!("   Spread: {}", spread);
            }
            if let Some(mid_price) = order_book.get_mid_price() {
                println!("   Mid price: {}", mid_price);
            }
        }
        None => {
            println!("📖 No order book data (expected without connection)");
        }
    }
    
    // Test best bid/ask
    match client.get_best_bid_ask("BTC/USD") {
        Some((bid, ask)) => {
            println!("💰 Best bid: {:?}, Best ask: {:?}", bid, ask);
        }
        None => {
            println!("💰 No bid/ask data (expected without connection)");
        }
    }
    
    // Demonstrate error handling
    println!("🧪 Testing error handling...");
    
    // Test with invalid channel
    let invalid_channels = vec![Channel::new("invalid_channel")];
    match client.subscribe(invalid_channels).await {
        Ok(_) => println!("❌ Invalid channel was unexpectedly accepted"),
        Err(e) => println!("✅ Invalid channel correctly rejected: {}", e),
    }
    
    // Cleanup and disconnect
    println!("🧹 Cleaning up...");
    client.cleanup().await?;
    
    println!("✅ Basic usage example completed successfully!");
    println!("💡 To test with real WebSocket connection, run the 'kraken_live_test' example");
    
    Ok(())
}