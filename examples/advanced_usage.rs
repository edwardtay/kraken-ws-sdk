//! Advanced usage example for the Kraken WebSocket SDK
//! Demonstrates error handling, custom callbacks, and performance optimization

use kraken_ws_sdk::{
    init_logging, Channel, ClientConfig, DataType, EventCallback, KrakenWsClient, 
    TickerData, TradeData, OrderBookUpdate, OHLCData, ConnectionState, SdkError,
    ReconnectConfig, ErrorReporter, ErrorContext
};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Duration;
use std::collections::HashMap;

/// Advanced callback with metrics and error handling
struct AdvancedCallback {
    ticker_count: AtomicU64,
    trade_count: AtomicU64,
    orderbook_count: AtomicU64,
    error_count: AtomicU64,
}

impl AdvancedCallback {
    fn new() -> Self {
        Self {
            ticker_count: AtomicU64::new(0),
            trade_count: AtomicU64::new(0),
            orderbook_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }
    
    fn print_stats(&self) {
        println!("📊 Message Statistics:");
        println!("   Tickers: {}", self.ticker_count.load(Ordering::Relaxed));
        println!("   Trades: {}", self.trade_count.load(Ordering::Relaxed));
        println!("   Order Books: {}", self.orderbook_count.load(Ordering::Relaxed));
        println!("   Errors: {}", self.error_count.load(Ordering::Relaxed));
    }
}

impl EventCallback for AdvancedCallback {
    fn on_ticker(&self, data: TickerData) {
        self.ticker_count.fetch_add(1, Ordering::Relaxed);
        
        // Advanced processing: calculate spread percentage
        let spread = &data.ask - &data.bid;
        let spread_pct = (&spread / &data.last_price) * rust_decimal::Decimal::from(100);
        
        if self.ticker_count.load(Ordering::Relaxed) % 10 == 0 {
            println!("📊 {} - Last: {}, Spread: {} ({:.4}%)", 
                data.symbol, data.last_price, spread, spread_pct);
        }
    }
    
    fn on_orderbook(&self, data: OrderBookUpdate) {
        self.orderbook_count.fetch_add(1, Ordering::Relaxed);
        
        // Advanced processing: analyze order book depth
        let total_bid_volume: rust_decimal::Decimal = data.bids.iter()
            .map(|level| level.volume)
            .sum();
        let total_ask_volume: rust_decimal::Decimal = data.asks.iter()
            .map(|level| level.volume)
            .sum();
        
        if self.orderbook_count.load(Ordering::Relaxed) % 5 == 0 {
            println!("📖 {} - Bid Volume: {}, Ask Volume: {}, Levels: {}/{}", 
                data.symbol, total_bid_volume, total_ask_volume, 
                data.bids.len(), data.asks.len());
        }
    }
    
    fn on_trade(&self, data: TradeData) {
        self.trade_count.fetch_add(1, Ordering::Relaxed);
        
        // Advanced processing: track large trades
        if data.volume > rust_decimal::Decimal::from(1) {
            println!("💰 Large Trade: {} {} @ {} ({})", 
                data.volume, data.symbol, data.price, 
                match data.side {
                    kraken_ws_sdk::TradeSide::Buy => "BUY",
                    kraken_ws_sdk::TradeSide::Sell => "SELL",
                });
        }
    }
    
    fn on_ohlc(&self, data: OHLCData) {
        // Calculate price change percentage
        let change = (&data.close - &data.open) / &data.open * rust_decimal::Decimal::from(100);
        println!("📈 {} [{}]: O:{} H:{} L:{} C:{} ({:+.2}%)", 
            data.symbol, data.interval, data.open, data.high, 
            data.low, data.close, change);
    }
    
    fn on_error(&self, error: SdkError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        
        // Advanced error handling with context
        let context = ErrorContext::new("callback_processing")
            .with_detail("error_type", &format!("{:?}", error))
            .with_detail("timestamp", &chrono::Utc::now().to_rfc3339());
        
        ErrorReporter::report_error(&error, Some(context));
        
        // Implement custom recovery logic based on error type
        match error {
            SdkError::Connection(_) => {
                println!("🔄 Connection error detected - client will attempt reconnection");
            }
            SdkError::Parse(_) => {
                println!("⚠️  Parse error - continuing with next message");
            }
            SdkError::Subscription(_) => {
                println!("📡 Subscription error - may need to resubscribe");
            }
            _ => {
                println!("❌ Other error: {}", error);
            }
        }
    }
    
    fn on_connection_state_change(&self, state: ConnectionState) {
        match state {
            ConnectionState::Connected => {
                println!("✅ Connected to Kraken WebSocket API");
            }
            ConnectionState::Connecting => {
                println!("🔌 Connecting to Kraken WebSocket API...");
            }
            ConnectionState::Disconnected => {
                println!("❌ Disconnected from Kraken WebSocket API");
            }
            ConnectionState::Reconnecting => {
                println!("🔄 Reconnecting to Kraken WebSocket API...");
            }
            ConnectionState::Failed => {
                println!("💥 Connection failed - manual intervention may be required");
            }
        }
    }
}

/// Performance monitoring callback
struct PerformanceCallback {
    start_time: std::time::Instant,
    message_count: AtomicU64,
}

impl PerformanceCallback {
    fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            message_count: AtomicU64::new(0),
        }
    }
    
    fn get_throughput(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let count = self.message_count.load(Ordering::Relaxed) as f64;
        if elapsed > 0.0 { count / elapsed } else { 0.0 }
    }
}

impl EventCallback for PerformanceCallback {
    fn on_ticker(&self, _data: TickerData) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_orderbook(&self, _data: OrderBookUpdate) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_trade(&self, _data: TradeData) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_ohlc(&self, _data: OHLCData) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn on_error(&self, _error: SdkError) {}
    
    fn on_connection_state_change(&self, _state: ConnectionState) {}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with custom configuration
    init_logging();
    
    println!("🦑 Kraken WebSocket SDK - Advanced Usage Example");
    
    // Create advanced client configuration with custom reconnection settings
    let config = ClientConfig {
        endpoint: "wss://ws.kraken.com".to_string(),
        timeout: Duration::from_secs(45),
        buffer_size: 2048, // Larger buffer for high-frequency data
        reconnect_config: ReconnectConfig {
            max_attempts: 15,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.5,
        },
        ..Default::default()
    };
    
    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("❌ Configuration validation failed: {}", e);
        return Ok(());
    }
    
    // Create the WebSocket client
    let mut client = KrakenWsClient::new(config);
    
    // Create multiple specialized callbacks
    let advanced_callback: Arc<dyn EventCallback> = Arc::new(AdvancedCallback::new());
    let performance_callback: Arc<dyn EventCallback> = Arc::new(PerformanceCallback::new());
    
    // Register multiple callbacks for the same data types
    let ticker_id1 = client.register_callback(DataType::Ticker, advanced_callback.clone());
    let ticker_id2 = client.register_callback(DataType::Ticker, performance_callback.clone());
    
    let trade_id1 = client.register_callback(DataType::Trade, advanced_callback.clone());
    let trade_id2 = client.register_callback(DataType::Trade, performance_callback.clone());
    
    let orderbook_id1 = client.register_callback(DataType::OrderBook, advanced_callback.clone());
    let orderbook_id2 = client.register_callback(DataType::OrderBook, performance_callback.clone());
    
    // Register connection listeners
    let connection_id = client.register_connection_listener(advanced_callback.clone());
    
    println!("✅ Registered multiple callbacks:");
    println!("   Ticker: {}, {}", ticker_id1, ticker_id2);
    println!("   Trade: {}, {}", trade_id1, trade_id2);
    println!("   OrderBook: {}, {}", orderbook_id1, orderbook_id2);
    println!("   Connection: {}", connection_id);
    
    // Connect to the WebSocket API
    println!("🔌 Connecting to Kraken WebSocket API...");
    if let Err(e) = client.connect().await {
        eprintln!("❌ Failed to connect: {}", e);
        return Ok(());
    }
    
    // Subscribe to multiple symbols and channels
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD"];
    let mut channels = Vec::new();
    
    for symbol in &symbols {
        channels.push(Channel::new("ticker").with_symbol(symbol));
        channels.push(Channel::new("trade").with_symbol(symbol));
        channels.push(Channel::new("book").with_symbol(symbol));
        channels.push(Channel::new("ohlc").with_symbol(symbol).with_interval("1"));
    }
    
    println!("📡 Subscribing to {} channels for {} symbols...", channels.len(), symbols.len());
    if let Err(e) = client.subscribe(channels).await {
        eprintln!("❌ Failed to subscribe: {}", e);
        return Ok(());
    }
    
    // Display initial status
    println!("🔗 Connection state: {:?}", client.connection_state());
    println!("📊 Active subscriptions: {:?}", client.get_active_subscriptions());
    
    // Run for extended period with periodic status updates
    let duration = Duration::from_secs(30);
    let start_time = std::time::Instant::now();
    
    println!("⏳ Running for {} seconds with periodic updates...", duration.as_secs());
    
    while start_time.elapsed() < duration {
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Print statistics (simplified - in real implementation you'd use proper callback management)
        println!("📊 Statistics updated (see callback implementations for details)");
        
        // Display order book information for each symbol
        for symbol in &symbols {
            if let Some(order_book) = client.get_order_book(symbol) {
                if let Some(spread) = order_book.get_spread() {
                    if let Some(mid_price) = order_book.get_mid_price() {
                        println!("📖 {}: Mid={}, Spread={}", symbol, mid_price, spread);
                    }
                }
            }
        }
        
        println!("---");
    }
    
    // Demonstrate callback management
    println!("🔧 Demonstrating callback unregistration...");
    
    // Unregister one of the ticker callbacks
    // Note: This would require implementing unregister methods in the client
    println!("   (Callback unregistration would be implemented here)");
    
    // Final statistics
    println!("📊 Final Statistics: (see callback output above)");
    
    // Cleanup and disconnect
    println!("🧹 Cleaning up...");
    client.cleanup().await?;
    
    println!("✅ Advanced example completed successfully!");
    Ok(())
}

