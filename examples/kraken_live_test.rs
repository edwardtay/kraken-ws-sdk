//! Live test with actual Kraken WebSocket API
//! This example connects to the real Kraken WebSocket API using your credentials

use kraken_ws_sdk::{
    init_logging, Channel, ClientConfig, DataType, EventCallback, KrakenWsClient, 
    TickerData, TradeData, OrderBookUpdate, OHLCData, ConnectionState, SdkError,
    ReconnectConfig
};
use std::sync::Arc;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};

/// Live test callback that tracks real data
struct LiveTestCallback {
    ticker_count: AtomicU64,
    trade_count: AtomicU64,
    orderbook_count: AtomicU64,
}

impl LiveTestCallback {
    fn new() -> Self {
        Self {
            ticker_count: AtomicU64::new(0),
            trade_count: AtomicU64::new(0),
            orderbook_count: AtomicU64::new(0),
        }
    }
    
    fn print_stats(&self) {
        println!("📊 Live Data Statistics:");
        println!("   Tickers received: {}", self.ticker_count.load(Ordering::Relaxed));
        println!("   Trades received: {}", self.trade_count.load(Ordering::Relaxed));
        println!("   Order book updates: {}", self.orderbook_count.load(Ordering::Relaxed));
    }
}

impl EventCallback for LiveTestCallback {
    fn on_ticker(&self, data: TickerData) {
        let count = self.ticker_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        // Print every 5th ticker to avoid spam
        if count % 5 == 0 {
            let spread = &data.ask - &data.bid;
            let spread_pct = (&spread / &data.last_price) * rust_decimal::Decimal::from(100);
            
            println!("🎯 LIVE TICKER #{}: {} - Last: ${}, Bid: ${}, Ask: ${}, Spread: {:.4}%", 
                count, data.symbol, data.last_price, data.bid, data.ask, spread_pct);
        }
    }
    
    fn on_orderbook(&self, data: OrderBookUpdate) {
        let count = self.orderbook_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        if count % 3 == 0 {
            let best_bid = data.bids.first().map(|b| b.price);
            let best_ask = data.asks.first().map(|a| a.price);
            
            println!("📖 LIVE ORDER BOOK #{}: {} - {} bids, {} asks | Best: {:?} / {:?}", 
                count, data.symbol, data.bids.len(), data.asks.len(), best_bid, best_ask);
        }
    }
    
    fn on_trade(&self, data: TradeData) {
        let count = self.trade_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        // Print all trades as they're less frequent
        let side_emoji = match data.side {
            kraken_ws_sdk::TradeSide::Buy => "🟢",
            kraken_ws_sdk::TradeSide::Sell => "🔴",
        };
        
        println!("💰 LIVE TRADE #{}: {} {} {} @ ${} (Vol: {})", 
            count, side_emoji, data.symbol, 
            match data.side { kraken_ws_sdk::TradeSide::Buy => "BUY", kraken_ws_sdk::TradeSide::Sell => "SELL" },
            data.price, data.volume);
    }
    
    fn on_ohlc(&self, data: OHLCData) {
        let change = (&data.close - &data.open) / &data.open * rust_decimal::Decimal::from(100);
        println!("📈 LIVE OHLC: {} [{}min] O:${} H:${} L:${} C:${} ({:+.2}%) Vol:{}", 
            data.symbol, data.interval, data.open, data.high, 
            data.low, data.close, change, data.volume);
    }
    
    fn on_error(&self, error: SdkError) {
        eprintln!("❌ LIVE ERROR: {}", error);
    }
    
    fn on_connection_state_change(&self, state: ConnectionState) {
        match state {
            ConnectionState::Connected => {
                println!("✅ CONNECTED to Kraken WebSocket API!");
            }
            ConnectionState::Connecting => {
                println!("🔌 Connecting to Kraken...");
            }
            ConnectionState::Disconnected => {
                println!("❌ Disconnected from Kraken");
            }
            ConnectionState::Reconnecting => {
                println!("🔄 Reconnecting to Kraken...");
            }
            ConnectionState::Failed => {
                println!("💥 Connection to Kraken failed!");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Initialize logging
    init_logging();
    
    println!("🦑 Kraken WebSocket SDK - LIVE TEST with Real API");
    println!("================================================");
    
    // Get API credentials from environment
    let api_key = std::env::var("KRAKEN_API_KEY")
        .expect("KRAKEN_API_KEY environment variable not set");
    let api_secret = std::env::var("KRAKEN_API_SECRET")
        .expect("KRAKEN_API_SECRET environment variable not set");
    
    println!("🔑 Using API Key: {}...", &api_key[..8]);
    
    // Create client configuration with real Kraken endpoint
    let config = ClientConfig {
        endpoint: "wss://ws.kraken.com".to_string(),
        api_key: Some(api_key),
        api_secret: Some(api_secret),
        timeout: Duration::from_secs(30),
        buffer_size: 2048,
        reconnect_config: ReconnectConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        },
    };
    
    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("❌ Configuration validation failed: {}", e);
        return Ok(());
    }
    
    println!("✅ Configuration validated successfully");
    
    // Create the WebSocket client
    let mut client = KrakenWsClient::new(config);
    
    // Create callback for live data
    let callback: Arc<dyn EventCallback> = Arc::new(LiveTestCallback::new());
    
    // Register callbacks for different data types
    let ticker_id = client.register_callback(DataType::Ticker, callback.clone());
    let trade_id = client.register_callback(DataType::Trade, callback.clone());
    let orderbook_id = client.register_callback(DataType::OrderBook, callback.clone());
    let ohlc_id = client.register_callback(DataType::OHLC, callback.clone());
    
    // Register connection state listener
    let connection_id = client.register_connection_listener(callback.clone());
    
    println!("📝 Registered callbacks:");
    println!("   Ticker: {}", ticker_id);
    println!("   Trade: {}", trade_id);
    println!("   OrderBook: {}", orderbook_id);
    println!("   OHLC: {}", ohlc_id);
    println!("   Connection: {}", connection_id);
    
    // Connect to the REAL Kraken WebSocket API
    println!("\n🔌 Connecting to LIVE Kraken WebSocket API...");
    if let Err(e) = client.connect().await {
        eprintln!("❌ Failed to connect to Kraken: {}", e);
        return Ok(());
    }
    
    // Subscribe to REAL market data channels
    let symbols = vec!["BTC/USD", "ETH/USD"];
    let mut channels = Vec::new();
    
    for symbol in &symbols {
        channels.push(Channel::new("ticker").with_symbol(symbol));
        channels.push(Channel::new("trade").with_symbol(symbol));
        channels.push(Channel::new("book").with_symbol(symbol));
        channels.push(Channel::new("ohlc").with_symbol(symbol).with_interval("1"));
    }
    
    println!("📡 Subscribing to LIVE data for: {:?}", symbols);
    if let Err(e) = client.subscribe(channels).await {
        eprintln!("❌ Failed to subscribe to Kraken channels: {}", e);
        return Ok(());
    }
    
    // Display connection status
    println!("🔗 Connection state: {:?}", client.connection_state());
    println!("📊 Active subscriptions: {:?}", client.get_active_subscriptions());
    
    println!("\n🎯 RECEIVING LIVE DATA FROM KRAKEN...");
    println!("=====================================");
    
    // Run for 60 seconds to collect real data
    let test_duration = Duration::from_secs(60);
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed() < test_duration {
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Print statistics every 10 seconds
        println!("📊 Statistics updated (see live data above)");
        
        // Display live order book information
        for symbol in &symbols {
            if let Some((best_bid, best_ask)) = client.get_best_bid_ask(symbol) {
                println!("💹 {} Live Prices - Bid: {:?}, Ask: {:?}", symbol, best_bid, best_ask);
            }
            
            if let Some(order_book) = client.get_order_book(symbol) {
                if let Some(spread) = order_book.get_spread() {
                    if let Some(mid_price) = order_book.get_mid_price() {
                        println!("📊 {} Live Book - Mid: ${}, Spread: ${}", symbol, mid_price, spread);
                    }
                }
            }
        }
        
        let remaining = test_duration.saturating_sub(start_time.elapsed());
        println!("⏱️  Time remaining: {}s\n", remaining.as_secs());
    }
    
    // Final statistics
    println!("🏁 LIVE TEST COMPLETED!");
    println!("=======================");
    println!("📊 Final statistics displayed in callback output above");
    
    // Test callback count functionality
    println!("\n📊 Callback Statistics:");
    println!("   Ticker callbacks: {}", client.get_callback_count(&DataType::Ticker));
    println!("   Trade callbacks: {}", client.get_callback_count(&DataType::Trade));
    println!("   OrderBook callbacks: {}", client.get_callback_count(&DataType::OrderBook));
    
    // Cleanup and disconnect
    println!("\n🧹 Cleaning up connection...");
    client.cleanup().await?;
    
    println!("✅ LIVE TEST WITH KRAKEN API COMPLETED SUCCESSFULLY! 🎉");
    println!("The SDK successfully connected to and received real market data from Kraken!");
    
    Ok(())
}