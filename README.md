# Kraken WebSocket SDK

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A lightweight, high-performance Rust SDK for connecting to Kraken's WebSocket API and processing real-time market data streams.

## Features

- 🚀 **High Performance**: Asynchronous processing with minimal overhead
- 🔒 **Type Safety**: Leverages Rust's type system for compile-time safety
- 🔄 **Auto Reconnection**: Intelligent reconnection with exponential backoff
- 📊 **Real-time Data**: Support for tickers, trades, order books, and OHLC data
- 🎯 **Event-Driven**: Flexible callback system for handling market data
- 🛡️ **Error Handling**: Comprehensive error handling and recovery
- 📈 **Order Book Management**: Built-in order book state management
- 🔧 **Configurable**: Extensive configuration options for production use

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
kraken-ws-sdk = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

### Basic Usage

```rust
use kraken_ws_sdk::{
    KrakenWsClient, ClientConfig, Channel, DataType, EventCallback,
    TickerData, TradeData, ConnectionState, SdkError
};
use std::sync::Arc;

struct MyCallback;

impl EventCallback for MyCallback {
    fn on_ticker(&self, data: TickerData) {
        println!("Ticker: {} - Last: {}", data.symbol, data.last_price);
    }
    
    fn on_trade(&self, data: TradeData) {
        println!("Trade: {} - {} @ {}", data.symbol, data.volume, data.price);
    }
    
    fn on_orderbook(&self, data: OrderBookUpdate) {
        println!("Order Book: {} - {} bids, {} asks", 
            data.symbol, data.bids.len(), data.asks.len());
    }
    
    fn on_ohlc(&self, data: OHLCData) {
        println!("OHLC: {} - Close: {}", data.symbol, data.close);
    }
    
    fn on_error(&self, error: SdkError) {
        eprintln!("Error: {}", error);
    }
    
    fn on_connection_state_change(&self, state: ConnectionState) {
        println!("Connection: {:?}", state);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let config = ClientConfig::default();
    let mut client = KrakenWsClient::new(config);
    
    // Register callback
    let callback: Arc<dyn EventCallback> = Arc::new(MyCallback);
    client.register_callback(DataType::Ticker, callback.clone());
    client.register_callback(DataType::Trade, callback);
    
    // Connect and subscribe
    client.connect().await?;
    
    let channels = vec![
        Channel::new("ticker").with_symbol("BTC/USD"),
        Channel::new("trade").with_symbol("BTC/USD"),
    ];
    client.subscribe(channels).await?;
    
    // Keep running
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    
    // Cleanup
    client.cleanup().await?;
    Ok(())
}
```

## Configuration

### Basic Configuration

```rust
use kraken_ws_sdk::{ClientConfig, ReconnectConfig};
use std::time::Duration;

let config = ClientConfig {
    endpoint: "wss://ws.kraken.com".to_string(),
    timeout: Duration::from_secs(30),
    buffer_size: 1024,
    reconnect_config: ReconnectConfig {
        max_attempts: 10,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
        backoff_multiplier: 2.0,
    },
    ..Default::default()
};
```

### Authentication (for private channels)

```rust
let config = ClientConfig {
    api_key: Some("your_api_key".to_string()),
    api_secret: Some("your_api_secret".to_string()),
    ..Default::default()
};
```

## Supported Channels

| Channel | Description | Public | Private |
|---------|-------------|--------|---------|
| `ticker` | Real-time ticker data | ✅ | ❌ |
| `trade` | Real-time trade data | ✅ | ❌ |
| `book` | Order book updates | ✅ | ❌ |
| `ohlc` | OHLC/candlestick data | ✅ | ❌ |
| `spread` | Spread data | ✅ | ❌ |
| `ownTrades` | User's trades | ❌ | ✅ |
| `openOrders` | User's open orders | ❌ | ✅ |

## Data Types

### TickerData

```rust
pub struct TickerData {
    pub symbol: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last_price: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
}
```

### TradeData

```rust
pub struct TradeData {
    pub symbol: String,
    pub price: Decimal,
    pub volume: Decimal,
    pub side: TradeSide,
    pub timestamp: DateTime<Utc>,
    pub trade_id: String,
}
```

### OrderBookUpdate

```rust
pub struct OrderBookUpdate {
    pub symbol: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub timestamp: DateTime<Utc>,
    pub checksum: Option<u32>,
}
```

## Error Handling

The SDK provides comprehensive error handling through the `SdkError` enum:

```rust
pub enum SdkError {
    Connection(ConnectionError),
    Parse(ParseError),
    Subscription(SubscriptionError),
    Configuration(String),
    Network(String),
    Authentication(String),
}
```

### Error Recovery

```rust
impl EventCallback for MyCallback {
    fn on_error(&self, error: SdkError) {
        match error {
            SdkError::Connection(conn_err) => {
                // Connection will auto-reconnect
                println!("Connection error: {}", conn_err);
            }
            SdkError::Parse(parse_err) => {
                // Parser will continue with next message
                println!("Parse error: {}", parse_err);
            }
            SdkError::Subscription(sub_err) => {
                // May need to resubscribe
                println!("Subscription error: {}", sub_err);
            }
            _ => {
                println!("Other error: {}", error);
            }
        }
    }
}
```

## Order Book Management

The SDK includes built-in order book state management:

```rust
// Get current order book
if let Some(order_book) = client.get_order_book("BTC/USD") {
    println!("Spread: {:?}", order_book.get_spread());
    println!("Mid price: {:?}", order_book.get_mid_price());
    
    let (bid_volume, ask_volume) = order_book.get_total_volume();
    println!("Total volume - Bids: {}, Asks: {}", bid_volume, ask_volume);
}

// Get best bid/ask
if let Some((best_bid, best_ask)) = client.get_best_bid_ask("BTC/USD") {
    println!("Best bid: {:?}, Best ask: {:?}", best_bid, best_ask);
}
```

## Advanced Features

### Multiple Callbacks

```rust
// Register multiple callbacks for the same data type
let callback1 = Arc::new(LoggingCallback);
let callback2 = Arc::new(MetricsCallback);

client.register_callback(DataType::Ticker, callback1);
client.register_callback(DataType::Ticker, callback2);
```

### Connection State Monitoring

```rust
client.register_connection_listener(Arc::new(ConnectionMonitor));

// Check connection state
match client.connection_state() {
    ConnectionState::Connected => println!("Connected"),
    ConnectionState::Reconnecting => println!("Reconnecting..."),
    ConnectionState::Failed => println!("Connection failed"),
    _ => {}
}
```

### Subscription Management

```rust
// Check active subscriptions
let subscriptions = client.get_active_subscriptions();
println!("Active subscriptions: {:?}", subscriptions);

// Check if subscribed to specific channel
let channel = Channel::new("ticker").with_symbol("BTC/USD");
if client.is_subscribed(&channel) {
    println!("Subscribed to BTC/USD ticker");
}
```

## Examples

See the `examples/` directory for complete examples:

- [`basic_usage.rs`](examples/basic_usage.rs) - Basic SDK usage
- [`advanced_usage.rs`](examples/advanced_usage.rs) - Advanced features and error handling
- [`web_demo/`](examples/web_demo/) - **Interactive web dashboard** with real-time market data

### Running Examples

**Command Line Examples:**
```bash
cargo run --example basic_usage
cargo run --example advanced_usage
```

**Web Demo Dashboard:**
```bash
# Option 1: Use the launcher script
./scripts/run_web_demo.sh

# Option 2: Run directly
cd examples/web_demo && cargo run
```

Then open your browser to: **http://localhost:3030**

### 🌐 Web Demo Features

The web demo provides a **beautiful, interactive dashboard** showcasing:

- **📊 Real-time Market Data** - Live cryptocurrency prices with animations
- **🔌 WebSocket Integration** - Demonstrates SDK's real-time capabilities  
- **📱 Responsive Design** - Works on desktop, tablet, and mobile
- **🔄 Auto Reconnection** - Shows SDK's connection resilience
- **📡 REST API** - Additional API endpoints for data access

![Web Demo Screenshot](examples/web_demo/screenshot.png)

## Performance

The SDK is designed for high-performance applications:

- **Asynchronous**: All operations are non-blocking
- **Zero-copy**: Minimal data copying where possible
- **Efficient parsing**: Optimized JSON parsing
- **Memory management**: Automatic cleanup and leak prevention
- **Concurrent processing**: Multiple callbacks can process data simultaneously

## Testing

Run tests with:

```bash
cargo test
```

The SDK includes both unit tests and property-based tests for comprehensive coverage.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This SDK is not officially affiliated with Kraken. Use at your own risk in production environments.

## Support

For questions and support:

- Check the [examples](examples/) directory
- Review the API documentation: `cargo doc --open`
- Open an issue on GitHub

---

Built with ❤️ in Rust