//! Web demo application for Kraken WebSocket SDK
//! 
//! This creates a simple web interface to demonstrate the SDK capabilities
//! including real-time market data display and WebSocket connectivity.

use kraken_ws_sdk::{
    init_logging, Channel, ClientConfig, DataType, EventCallback, KrakenWsClient,
    TickerData, TradeData, OrderBookUpdate, OHLCData, ConnectionState, SdkError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use warp::Filter;
use futures_util::{SinkExt, StreamExt};

/// Market data for web display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub last_price: Option<String>,
    pub bid: Option<String>,
    pub ask: Option<String>,
    pub volume: Option<String>,
    pub spread: Option<String>,
    pub last_trade: Option<TradeInfo>,
    pub connection_status: String,
    pub timestamp: String,
    /// Exchange timestamp (when Kraken generated the message) - for latency tracking
    pub exchange_timestamp: Option<String>,
    // Backpressure stats
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub messages_coalesced: u64,
    pub current_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInfo {
    pub price: String,
    pub volume: String,
    pub side: String,
    pub timestamp: String,
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub market_data: Arc<Mutex<HashMap<String, MarketData>>>,
    pub broadcast_tx: broadcast::Sender<MarketData>,
}

/// WebSocket callback that updates the web interface
pub struct WebCallback {
    state: AppState,
}

impl WebCallback {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}
impl EventCallback for WebCallback {
    fn on_ticker(&self, data: TickerData) {
        let mut market_data = self.state.market_data.lock().unwrap();
        let entry = market_data.entry(data.symbol.clone()).or_insert_with(|| MarketData {
            symbol: data.symbol.clone(),
            last_price: None,
            bid: None,
            ask: None,
            volume: None,
            spread: None,
            last_trade: None,
            connection_status: "Connected".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            exchange_timestamp: None,
            messages_received: 0,
            messages_dropped: 0,
            messages_coalesced: 0,
            current_rate: 0.0,
        });
        
        entry.last_price = Some(data.last_price.to_string());
        entry.bid = Some(data.bid.to_string());
        entry.ask = Some(data.ask.to_string());
        entry.volume = Some(data.volume.to_string());
        entry.spread = Some((data.ask - data.bid).to_string());
        entry.timestamp = chrono::Utc::now().to_rfc3339();
        // Exchange timestamp for latency tracking
        entry.exchange_timestamp = Some(data.timestamp.to_rfc3339());
        entry.messages_received += 1;
        
        // Broadcast update to WebSocket clients
        let _ = self.state.broadcast_tx.send(entry.clone());
        
        println!("📊 Updated ticker for {}: ${}", data.symbol, data.last_price);
    }
    
    fn on_trade(&self, data: TradeData) {
        let mut market_data = self.state.market_data.lock().unwrap();
        let entry = market_data.entry(data.symbol.clone()).or_insert_with(|| MarketData {
            symbol: data.symbol.clone(),
            last_price: None,
            bid: None,
            ask: None,
            volume: None,
            spread: None,
            last_trade: None,
            connection_status: "Connected".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            exchange_timestamp: None,
            messages_received: 0,
            messages_dropped: 0,
            messages_coalesced: 0,
            current_rate: 0.0,
        });
        
        entry.last_trade = Some(TradeInfo {
            price: data.price.to_string(),
            volume: data.volume.to_string(),
            side: format!("{:?}", data.side),
            timestamp: data.timestamp.to_rfc3339(),
        });
        entry.timestamp = chrono::Utc::now().to_rfc3339();
        // Exchange timestamp for latency tracking
        entry.exchange_timestamp = Some(data.timestamp.to_rfc3339());
        entry.messages_received += 1;
        
        // Broadcast update to WebSocket clients
        let _ = self.state.broadcast_tx.send(entry.clone());
        
        println!("💰 New trade for {}: {} {} @ ${}", 
            data.symbol, data.volume, format!("{:?}", data.side), data.price);
    }
    
    fn on_orderbook(&self, data: OrderBookUpdate) {
        println!("📖 Order book update for {}: {} bids, {} asks", 
            data.symbol, data.bids.len(), data.asks.len());
    }
    
    fn on_ohlc(&self, data: OHLCData) {
        println!("📈 OHLC update for {}: O:{} H:{} L:{} C:{}", 
            data.symbol, data.open, data.high, data.low, data.close);
    }
    
    fn on_error(&self, error: SdkError) {
        eprintln!("❌ SDK Error: {}", error);
        
        // Update connection status for all symbols
        let mut market_data = self.state.market_data.lock().unwrap();
        for (_, data) in market_data.iter_mut() {
            data.connection_status = format!("Error: {}", error);
        }
    }
    
    fn on_connection_state_change(&self, state: ConnectionState) {
        println!("🔗 Connection state changed: {:?}", state);
        
        let status = match state {
            ConnectionState::Connected => "Connected",
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Reconnecting => "Reconnecting",
            ConnectionState::Failed => "Failed",
        };
        
        // Update connection status for all symbols
        let mut market_data = self.state.market_data.lock().unwrap();
        for (_, data) in market_data.iter_mut() {
            data.connection_status = status.to_string();
        }
    }
}
/// WebSocket handler for real-time updates
async fn websocket_handler(
    ws: warp::ws::Ws,
    state: AppState,
) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(ws.on_upgrade(move |socket| websocket_connection(socket, state)))
}

/// Message from frontend to request subscription
#[derive(Debug, Deserialize)]
struct ClientMessage {
    #[serde(rename = "type")]
    msg_type: String,
    symbol: Option<String>,
}

async fn websocket_connection(ws: warp::ws::WebSocket, state: AppState) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    
    // Send current market data on connection
    let initial_data = {
        let market_data = state.market_data.lock().unwrap();
        market_data.values().cloned().collect::<Vec<_>>()
    };
    
    for data in initial_data {
        let message = serde_json::to_string(&data).unwrap();
        if ws_tx.send(warp::ws::Message::text(message)).await.is_err() {
            return;
        }
    }
    
    // Handle incoming and outgoing messages concurrently
    tokio::select! {
        _ = async {
            // Listen for broadcast updates
            while let Ok(data) = broadcast_rx.recv().await {
                let message = serde_json::to_string(&data).unwrap();
                if ws_tx.send(warp::ws::Message::text(message)).await.is_err() {
                    break;
                }
            }
        } => {},
        _ = async {
            // Listen for client messages
            while let Some(result) = ws_rx.next().await {
                match result {
                    Ok(msg) => {
                        if msg.is_text() {
                            if let Ok(text) = msg.to_str() {
                                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(text) {
                                    if client_msg.msg_type == "subscribe" {
                                        if let Some(symbol) = client_msg.symbol {
                                            println!("📡 Client requested subscription to: {}", symbol);
                                            // Initialize market data entry for new symbol
                                            let mut market_data = state.market_data.lock().unwrap();
                                            if !market_data.contains_key(&symbol) {
                                                market_data.insert(symbol.clone(), MarketData {
                                                    symbol: symbol.clone(),
                                                    last_price: Some("--".to_string()),
                                                    bid: Some("--".to_string()),
                                                    ask: Some("--".to_string()),
                                                    volume: Some("--".to_string()),
                                                    spread: Some("--".to_string()),
                                                    last_trade: None,
                                                    connection_status: "Subscribing...".to_string(),
                                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                                    exchange_timestamp: None,
                                                    messages_received: 0,
                                                    messages_dropped: 0,
                                                    messages_coalesced: 0,
                                                    current_rate: 0.0,
                                                });
                                                // Broadcast the new entry immediately
                                                let entry = market_data.get(&symbol).unwrap().clone();
                                                let _ = state.broadcast_tx.send(entry);
                                            }
                                        }
                                    } else if client_msg.msg_type == "unsubscribe" {
                                        if let Some(symbol) = client_msg.symbol {
                                            println!("📡 Client requested unsubscribe from: {}", symbol);
                                            let mut market_data = state.market_data.lock().unwrap();
                                            market_data.remove(&symbol);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        } => {},
    }
}

/// API endpoint to get current market data
async fn get_market_data(state: AppState) -> Result<impl warp::Reply, warp::Rejection> {
    let market_data = state.market_data.lock().unwrap();
    let data: Vec<_> = market_data.values().cloned().collect();
    Ok(warp::reply::json(&data))
}

/// Health check endpoint
async fn health_check() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "service": "kraken-ws-demo",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging();
    
    println!("🦑 Kraken WebSocket SDK - Web Demo");
    println!("==================================");
    
    // Create application state
    let (broadcast_tx, _) = broadcast::channel(100);
    let state = AppState {
        market_data: Arc::new(Mutex::new(HashMap::new())),
        broadcast_tx,
    };
    
    // Initialize some demo data
    {
        let mut market_data = state.market_data.lock().unwrap();
        for symbol in &["BTC/USD", "ETH/USD", "SOL/USD"] {
            market_data.insert(symbol.to_string(), MarketData {
                symbol: symbol.to_string(),
                last_price: Some("0.00".to_string()),
                bid: Some("0.00".to_string()),
                ask: Some("0.00".to_string()),
                volume: Some("0.00".to_string()),
                spread: Some("0.00".to_string()),
                last_trade: None,
                connection_status: "Initializing".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                exchange_timestamp: None,
                messages_received: 0,
                messages_dropped: 0,
                messages_coalesced: 0,
                current_rate: 0.0,
            });
        }
    }
    
    // Create Kraken WebSocket client
    let config = ClientConfig {
        endpoint: "wss://ws.kraken.com".to_string(),
        timeout: Duration::from_secs(30),
        buffer_size: 2048,
        ..Default::default()
    };
    
    let mut client = KrakenWsClient::new(config);
    let callback = Arc::new(WebCallback::new(state.clone()));
    
    // Register callbacks
    client.register_callback(DataType::Ticker, callback.clone());
    client.register_callback(DataType::Trade, callback.clone());
    client.register_callback(DataType::OrderBook, callback.clone());
    client.register_connection_listener(callback);
    
    // Start WebSocket client in background
    let client_state = state.clone();
    tokio::spawn(async move {
        println!("🔌 Starting Kraken WebSocket connection...");
        
        // Subscribe to real market data channels BEFORE connecting
        let channels = vec![
            Channel::new("ticker").with_symbol("BTC/USD"),   // BTC/USD 
            Channel::new("ticker").with_symbol("ETH/USD"),   // ETH/USD
            Channel::new("ticker").with_symbol("SOL/USD"),   // SOL/USD
            Channel::new("trade").with_symbol("BTC/USD"),    // BTC trades
        ];
        
        if let Err(e) = client.subscribe(channels).await {
            eprintln!("❌ Failed to prepare subscription: {}", e);
            println!("📊 Falling back to simulated market data");
            simulate_market_data(client_state).await;
            return;
        }
        
        // Connect to real Kraken WebSocket API (this will send the subscription)
        if let Err(e) = client.connect().await {
            eprintln!("❌ Failed to connect to Kraken: {}", e);
            // Fall back to simulation if connection fails
            println!("📊 Falling back to simulated market data");
            simulate_market_data(client_state).await;
            return;
        }
        
        println!("✅ Connected and subscribed to live Kraken market data channels");
        println!("🔄 Receiving live data from Kraken WebSocket API");
        
        // Keep the connection alive
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            println!("💓 WebSocket connection heartbeat");
        }
    });
    
    // Set up web server routes
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "OPTIONS"]);
    
    // API routes
    let api_state = state.clone();
    let api_routes = warp::path("api")
        .and(
            warp::path("market-data")
                .and(warp::get())
                .and(warp::any().map(move || api_state.clone()))
                .and_then(get_market_data)
                .or(
                    warp::path("health")
                        .and(warp::get())
                        .and_then(health_check)
                )
        );
    
    // WebSocket route
    let ws_state = state.clone();
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::any().map(move || ws_state.clone()))
        .and_then(websocket_handler);
    
    // Static file serving
    let static_files = warp::path("static")
        .and(warp::fs::dir("static"));
    
    // Main page
    let index = warp::path::end()
        .and(warp::get())
        .and(warp::fs::file("static/index.html"));
    
    let routes = index
        .or(static_files)
        .or(api_routes)
        .or(ws_route)
        .with(cors);
    
    println!("🌐 Starting web server on http://localhost:3032");
    println!("📊 Market data dashboard: http://localhost:3032");
    println!("🔌 WebSocket endpoint: ws://localhost:3032/ws");
    println!("📡 API endpoint: http://localhost:3032/api/market-data");
    
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3032))
        .await;
    
    Ok(())
}
/// Simulate market data for demo purposes
async fn simulate_market_data(state: AppState) {
    use rust_decimal::Decimal;
    use std::str::FromStr;
    
    let symbols = vec!["BTC/USD", "ETH/USD", "SOL/USD"];
    let mut prices = HashMap::new();
    
    // Initialize prices
    prices.insert("BTC/USD", Decimal::from_str("45000.00").unwrap());
    prices.insert("ETH/USD", Decimal::from_str("3000.00").unwrap());
    prices.insert("ADA/USD", Decimal::from_str("0.50").unwrap());
    
    let mut counter = 0;
    
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        for symbol in &symbols {
            let current_price = prices.get_mut(symbol).unwrap();
            
            // Simulate price movement (±2% random walk)
            let change_percent = (rand::random() - 0.5) * 0.04; // ±2%
            let change = *current_price * Decimal::from_str(&change_percent.to_string()).unwrap_or_default();
            *current_price += change;
            
            // Ensure price stays positive
            if *current_price <= Decimal::ZERO {
                *current_price = Decimal::from_str("1.00").unwrap();
            }
            
            let bid = *current_price * Decimal::from_str("0.999").unwrap();
            let ask = *current_price * Decimal::from_str("1.001").unwrap();
            let volume = Decimal::from_str(&(rand::random() * 1000.0).to_string()).unwrap_or_default();
            
            // Create ticker data
            let ticker_data = TickerData {
                symbol: symbol.to_string(),
                bid,
                ask,
                last_price: *current_price,
                volume,
                timestamp: chrono::Utc::now(),
            };
            
            // Update market data
            let mut market_data = state.market_data.lock().unwrap();
            let entry = market_data.entry(symbol.to_string()).or_insert_with(|| MarketData {
                symbol: symbol.to_string(),
                last_price: None,
                bid: None,
                ask: None,
                volume: None,
                spread: None,
                last_trade: None,
                connection_status: "Connected (Simulated)".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                exchange_timestamp: None,
                messages_received: 0,
                messages_dropped: 0,
                messages_coalesced: 0,
                current_rate: 0.0,
            });
            
            entry.last_price = Some(current_price.to_string());
            entry.bid = Some(bid.to_string());
            entry.ask = Some(ask.to_string());
            entry.volume = Some(volume.to_string());
            entry.spread = Some((ask - bid).to_string());
            entry.timestamp = chrono::Utc::now().to_rfc3339();
            entry.exchange_timestamp = Some(chrono::Utc::now().to_rfc3339());
            entry.messages_received += 1;
            
            // Broadcast update
            let _ = state.broadcast_tx.send(entry.clone());
            
            // Occasionally simulate a trade
            if counter % 5 == 0 {
                let trade_data = TradeData {
                    symbol: symbol.to_string(),
                    price: *current_price,
                    volume: Decimal::from_str(&(rand::random() * 10.0).to_string()).unwrap_or_default(),
                    side: if rand::random() > 0.5 { 
                        kraken_ws_sdk::TradeSide::Buy 
                    } else { 
                        kraken_ws_sdk::TradeSide::Sell 
                    },
                    timestamp: chrono::Utc::now(),
                    trade_id: uuid::Uuid::new_v4().to_string(),
                };
                
                entry.last_trade = Some(TradeInfo {
                    price: trade_data.price.to_string(),
                    volume: trade_data.volume.to_string(),
                    side: format!("{:?}", trade_data.side),
                    timestamp: trade_data.timestamp.to_rfc3339(),
                });
                
                let _ = state.broadcast_tx.send(entry.clone());
            }
        }
        
        counter += 1;
    }
}

// Simple random number generation for demo
mod rand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    pub fn random() -> f64 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().hash(&mut hasher);
        (hasher.finish() % 1000000) as f64 / 1000000.0
    }
}