# Kraken WebSocket SDK

[![Crates.io](https://img.shields.io/crates/v/kraken-ws-sdk.svg)](https://crates.io/crates/kraken-ws-sdk)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![docs.rs](https://docs.rs/kraken-ws-sdk/badge.svg)](https://docs.rs/kraken-ws-sdk)

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

Then open your browser to: **http://localhost:3032**

### 🌐 Web Demo Features

The web demo provides a **beautiful, interactive dashboard** showcasing:

- **📊 Real-time Market Data** - Live cryptocurrency prices with animations
- **🔌 WebSocket Integration** - Demonstrates SDK's real-time capabilities  
- **📱 Responsive Design** - Works on desktop, tablet, and mobile
- **🔄 Auto Reconnection** - Shows SDK's connection resilience
- **📡 REST API** - Additional API endpoints for data access

![Web Demo Screenshot](examples/web_demo/screenshot.png)

## Correctness Contract

### Reconnection Behavior

| Event | SDK Behavior | User Action Required |
|-------|--------------|---------------------|
| Network disconnect | Auto-reconnect with exponential backoff (100ms → 30s) | None |
| Server close (1000) | Reconnect after `initial_delay` | None |
| Auth failure | Stop reconnecting, emit error | Re-authenticate |
| Max attempts reached | Emit `ConnectionState::Failed` | Manual `connect()` |

**On successful reconnect:**
1. All previous subscriptions are automatically restored
2. `on_reconnect(attempt_number)` callback fires
3. Order book state is invalidated (snapshot required)

### Sequence Gap Handling

```
Expected: seq 100 → Received: seq 105
         ↓
    Gap detected (size: 5)
         ↓
    on_gap_detected(expected=100, received=105)
         ↓
    Resync triggered (request snapshot)
         ↓
    on_resync(reason=GapDetected)
```

**Gap policies:**
- `GapPolicy::Resync` (default) - Request fresh snapshot on any gap
- `GapPolicy::Ignore` - Continue processing, accept data loss
- `GapPolicy::Buffer` - Buffer messages, attempt reorder within window

### Order Book Stitching Rules

1. **Snapshot first**: Always wait for snapshot before applying deltas
2. **Checksum validation**: Verify CRC32 checksum after each update (if provided)
3. **Sequence ordering**: Deltas must be applied in sequence order
4. **Stale detection**: Discard deltas older than current snapshot

```rust
// Stitching state machine
Disconnected → Snapshot Received → Applying Deltas → Checksum Valid ✓
                     ↑                    ↓
                     └── Checksum Fail ───┘ (request new snapshot)
```

### Timestamp Guarantees

| Guarantee | Level |
|-----------|-------|
| Exchange timestamps | **Monotonic per symbol** (Kraken guarantees) |
| Receive timestamps | **Best effort** (network jitter possible) |
| Latency calculation | `receive_time - exchange_time` (clock sync dependent) |

**Note:** For accurate latency, ensure NTP sync on your system.

---

## Tuning Guide

### Buffer Sizes

| Use Case | `buffer_size` | `max_queue_depth` | Notes |
|----------|---------------|-------------------|-------|
| Single pair, low freq | 64 | 100 | Minimal memory |
| Single pair, high freq | 256 | 500 | BTC/USD during volatility |
| 10 pairs, mixed freq | 512 | 1000 | Typical trading bot |
| 50+ pairs, all tickers | 2048 | 5000 | Market maker / aggregator |
| Order book depth 1000 | 4096 | 2000 | Deep book tracking |

### Backpressure Configuration

```rust
BackpressureConfig {
    max_messages_per_second: 1000,  // Rate limit
    max_queue_depth: 500,           // Buffer before dropping
    drop_policy: DropPolicy::Oldest,  // What to drop
    coalesce_window_ms: 10,         // Merge window for same-symbol updates
}
```

**Drop Policies:**
| Policy | Behavior | Best For |
|--------|----------|----------|
| `Oldest` | Remove oldest queued message | Real-time displays |
| `Latest` | Reject incoming message | Audit/logging |
| `Coalesce` | Merge updates for same symbol | High-frequency tickers |

**Dropped vs Coalesced:**
- **Dropped**: Message discarded entirely, data loss
- **Coalesced**: Multiple updates merged into one, no data loss but reduced granularity

### Recommended Defaults

**Low-latency trading (single pair):**
```rust
ClientConfig {
    buffer_size: 128,
    timeout: Duration::from_millis(5000),
    ..Default::default()
}
BackpressureConfig {
    max_messages_per_second: 0,  // No limit
    drop_policy: DropPolicy::Oldest,
    ..Default::default()
}
```

**Multi-pair monitoring (10-50 pairs):**
```rust
ClientConfig {
    buffer_size: 1024,
    timeout: Duration::from_secs(30),
    ..Default::default()
}
BackpressureConfig {
    max_messages_per_second: 5000,
    coalesce_window_ms: 50,
    drop_policy: DropPolicy::Coalesce,
    ..Default::default()
}
```

**High-frequency aggregator (100+ pairs):**
```rust
ClientConfig {
    buffer_size: 4096,
    timeout: Duration::from_secs(60),
    ..Default::default()
}
BackpressureConfig {
    max_messages_per_second: 10000,
    max_queue_depth: 10000,
    coalesce_window_ms: 100,
    drop_policy: DropPolicy::Coalesce,
    ..Default::default()
}
```

---

## Feature Flags

```toml
[dependencies]
# Minimal - public market data only
kraken-ws-sdk = "0.1"

# With private channels (requires API key)
kraken-ws-sdk = { version = "0.1", features = ["private"] }

# Full orderbook state management
kraken-ws-sdk = { version = "0.1", features = ["orderbook-state"] }

# Everything
kraken-ws-sdk = { version = "0.1", features = ["full"] }

# WebAssembly target
kraken-ws-sdk = { version = "0.1", default-features = false, features = ["wasm"] }
```

| Feature | Description | Dependencies Added |
|---------|-------------|-------------------|
| `public` (default) | Ticker, trades, book, OHLC | Core only |
| `private` | ownTrades, openOrders | Auth modules |
| `orderbook-state` | Full book management | CRC32, state machine |
| `metrics` | Prometheus export | prometheus crate |
| `chaos` | Fault injection | None |
| `wasm` | Browser support | wasm-bindgen |

---

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

## Security

### Credential Handling

**DO NOT:**
- Log API keys or secrets (SDK redacts these automatically)
- Commit `.env` files (use `.env.example` as template)
- Pass credentials in URLs or query strings
- Store credentials in code or version control

**DO:**
- Use environment variables for credentials
- Use `.env` files locally (gitignored)
- Use secret managers in production (AWS Secrets Manager, Vault, etc.)

```rust
// ✅ GOOD: Load from environment
let api_key = std::env::var("KRAKEN_API_KEY").ok();
let api_secret = std::env::var("KRAKEN_API_SECRET").ok();

// ❌ BAD: Hardcoded credentials
let api_key = Some("abc123".to_string()); // NEVER DO THIS
```

### What the SDK Logs

| Data | Logged? | Notes |
|------|---------|-------|
| API Key | ❌ Never | Redacted to `***` |
| API Secret | ❌ Never | Never appears in logs |
| Auth tokens | ❌ Never | Generated internally, not logged |
| Symbols/pairs | ✅ Yes | e.g., "BTC/USD" |
| Prices/volumes | ✅ Yes | Market data is public |
| Connection state | ✅ Yes | "Connected", "Reconnecting" |
| Error messages | ✅ Yes | Without sensitive data |

### Network Security

- All connections use **TLS 1.2+** (wss://)
- Certificate validation is enabled by default
- No plaintext WebSocket (ws://) in production

### Reporting Vulnerabilities

If you discover a security vulnerability, please:
1. **Do not** open a public issue
2. Email security concerns to the maintainers
3. Allow 90 days for a fix before public disclosure

---

## Disclaimer

This SDK is not officially affiliated with Kraken. Use at your own risk in production environments.

## Support

For questions and support:

- Check the [examples](examples/) directory
- Review the API documentation: `cargo doc --open`
- Open an issue on GitHub

---

Built with ❤️ in Rust