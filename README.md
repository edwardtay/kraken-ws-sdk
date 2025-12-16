# Kraken WebSocket SDK

[![Crates.io](https://img.shields.io/crates/v/kraken-ws-sdk.svg)](https://crates.io/crates/kraken-ws-sdk)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![docs.rs](https://docs.rs/kraken-ws-sdk/badge.svg)](https://docs.rs/kraken-ws-sdk)

A production-grade, correctness-first Kraken WebSocket SDK in Rust, designed for reliable market data ingestion and long-running trading systems.

> **Note:** This is an independent, community-maintained SDK. Not affiliated with Kraken.

## Who This Is For

- **Production market data ingestion** - reliable, 24/7 data pipelines
- **Research and strategy development** - backtesting, signal generation
- **Trading system infrastructure** - not a full trading engine, but the data layer for one

## What Is Stable Today

| Component | Status | Notes |
|-----------|--------|-------|
| Public market data | ✅ **Stable** | Ticker, trades, orderbook, OHLC |
| Reconnect semantics | ✅ **Stable** | Deterministic state machine, auto-resubscribe |
| Orderbook stitching | ✅ **Stable** | Snapshot + delta, checksum validation |
| `prelude` module | ✅ **Stable** | Won't break in minor versions |

## What Is NOT Frozen Yet

| Component | Status | Notes |
|-----------|--------|-------|
| Auth channels | ⚠️ **Unstable** | `ownTrades`, `openOrders` - API may change |
| Config surface | ⚠️ **Unstable** | New fields may be added (with defaults) |
| Metrics shape | ⚠️ **Unstable** | Prometheus export format may evolve |
| `extended` module | ⚠️ **Growing** | Stable but new types may be added |

---

## API Stability

This SDK follows a **frozen API** philosophy for production reliability:

| Module | Stability | Description |
|--------|-----------|-------------|
| `prelude` | **Stable** | Core API - won't break between minor versions |
| `extended` | **Stable** | Advanced features - stable but may grow |
| `internal` | **Unstable** | Implementation details - may change |

```rust
// ✅ Use this for production code
use kraken_ws_sdk::prelude::*;

// ✅ For advanced features
use kraken_ws_sdk::extended::*;

// ❌ Don't depend on internal modules
// use kraken_ws_sdk::internal::*;
```

## Features

- 🔒 **Frozen API**: Minimal, stable surface - trading firms hate churn
- 🎯 **Deterministic State Machine**: Explicit connection states with single-cause transitions
- 🚀 **High Performance**: Async processing with minimal overhead
- 📊 **Real-time Data**: Tickers, trades, order books, OHLC
- 🔄 **Auto Recovery**: Exponential backoff with configurable retry limits
- 📈 **Order Book Management**: Snapshot + delta stitching with checksum validation

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
kraken-ws-sdk = "0.2"
tokio = { version = "1.0", features = ["full"] }
```

### Stream API (Recommended)

The simplest way to consume events - a single unified stream:

```rust
use kraken_ws_sdk::{KrakenWsClient, ClientConfig, Channel, SdkEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KrakenWsClient::new(ClientConfig::default());
    
    // Get unified event stream
    let mut events = client.events();
    
    // Subscribe and connect
    client.subscribe(vec![
        Channel::new("ticker").with_symbol("BTC/USD"),
        Channel::new("trade").with_symbol("BTC/USD"),
    ]).await?;
    client.connect().await?;
    
    // Process all events in one place
    while let Some(event) = events.recv().await {
        match event {
            SdkEvent::Ticker(t) => println!("📊 {}: ${}", t.symbol, t.last_price),
            SdkEvent::Trade(t) => println!("💰 {}: {} @ ${}", t.symbol, t.volume, t.price),
            SdkEvent::OrderBook(b) => println!("📖 {}: {} bids", b.symbol, b.bids.len()),
            SdkEvent::State(s) => println!("🔗 Connection: {:?}", s),
            SdkEvent::Error(e) => eprintln!("❌ Error: {}", e),
            _ => {}
        }
    }
    Ok(())
}
```

### Callback API (Traditional)

For more control, register callbacks per data type:

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
    let mut client = KrakenWsClient::new(ClientConfig::default());
    
    // Register callbacks
    let callback: Arc<dyn EventCallback> = Arc::new(MyCallback);
    client.register_callback(DataType::Ticker, callback.clone());
    client.register_callback(DataType::Trade, callback);
    
    // Connect and subscribe
    client.subscribe(vec![
        Channel::new("ticker").with_symbol("BTC/USD"),
        Channel::new("trade").with_symbol("BTC/USD"),
    ]).await?;
    client.connect().await?;
    
    // Keep running
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
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

See the `examples/` directory for SDK usage examples:

```bash
cargo run --example basic_usage
cargo run --example advanced_usage
cargo run --example backpressure_demo
cargo run --example latency_demo
```

| Example | Description |
|---------|-------------|
| `basic_usage.rs` | Minimal SDK setup, subscribe to ticker |
| `advanced_usage.rs` | Error handling, multiple callbacks |
| `backpressure_demo.rs` | Rate limiting, drop policies |
| `latency_demo.rs` | Latency tracking, percentiles |
| `sequencing_demo.rs` | Gap detection, resync handling |

### Demo Applications

For interactive demos (not production code), see the [`demo/`](demo/) folder:
- `demo/web_demo/` - Web-based SDK observability console
- `demo/orderbook-visualizer/` - React orderbook visualization

> ⚠️ **Demo apps are for learning only** - no auth keys, no production claims.

## Connection State Machine

The SDK uses a **deterministic state machine** for connection management. Each state has explicit transitions with single causes and actions.

```
┌─────────────┐                    ┌─────────────┐                    ┌───────────────┐
│ DISCONNECTED│───── connect() ───▶│  CONNECTING │───── success ─────▶│AUTHENTICATING │
└─────────────┘                    └─────────────┘                    └───────────────┘
       ▲                                  │                                   │
       │                               failure                          success/skip
       │                                  │                                   │
       │                                  ▼                                   ▼
       │                           ┌──────────┐                        ┌─────────────┐
       │                           │ DEGRADED │◀─── subscription ──────│ SUBSCRIBING │
       │                           └──────────┘      failed            └─────────────┘
       │                                  │                                   │
       │                               retry                              success
       │                                  │                                   │
       │                                  ▼                                   ▼
       │                           ┌──────────┐    gap_detected       ┌────────────┐
       └────── close() ────────────│  CLOSED  │◀───────────────────────│ SUBSCRIBED │
                                   └──────────┘                        └────────────┘
                                        ▲                                    │
                                        │                              gap_detected
                                   max_retries                               │
                                        │                                    ▼
                                        └──────────────────────────────┌───────────┐
                                                                       │ RESYNCING │
                                                                       └───────────┘
```

### State Descriptions

| State | Description | Exit Conditions |
|-------|-------------|-----------------|
| `DISCONNECTED` | Initial state | `connect()` → CONNECTING |
| `CONNECTING` | Establishing WebSocket | success → AUTHENTICATING, failure → DEGRADED |
| `AUTHENTICATING` | Sending API credentials | success → SUBSCRIBING, failure → DEGRADED |
| `SUBSCRIBING` | Sending subscription requests | all confirmed → SUBSCRIBED, failure → DEGRADED |
| `SUBSCRIBED` | Receiving data normally | gap → RESYNCING, disconnect → DEGRADED, `close()` → CLOSED |
| `RESYNCING` | Recovering from sequence gap | complete → SUBSCRIBED, failure → DEGRADED |
| `DEGRADED` | Attempting recovery | retry → CONNECTING, max_retries → CLOSED |
| `CLOSED` | Terminal state | `connect()` starts new connection |

### State Events

Every state transition emits an `Event::StateChange(ConnectionState)`:

```rust
use kraken_ws_sdk::prelude::*;

let mut events = client.events();
while let Some(event) = events.recv().await {
    match event {
        Event::StateChange(state) => {
            match state {
                ConnectionState::Subscribed => println!("✅ Ready to receive data"),
                ConnectionState::Degraded { reason, retry_count, .. } => {
                    println!("⚠️ Degraded: {:?}, retry #{}", reason, retry_count);
                }
                ConnectionState::Closed { reason } => {
                    println!("❌ Closed: {:?}", reason);
                    break;
                }
                _ => {}
            }
        }
        _ => {}
    }
}
```

## Correctness Guarantees

This section defines the SDK's behavioral contract. These are guarantees, not just features.

### Message Ordering

| Scope | Guarantee | Notes |
|-------|-----------|-------|
| Per symbol, per channel | **Strictly ordered** | Ticker updates for BTC/USD arrive in exchange order |
| Per symbol, across channels | **No guarantee** | Ticker and trade for same symbol may interleave |
| Across symbols | **No guarantee** | BTC and ETH updates are independent streams |

**Kraken's guarantee:** Messages within a channel/pair are sequenced. The SDK preserves this ordering.

### Heartbeat & Liveness

| Mechanism | Interval | SDK Behavior |
|-----------|----------|--------------|
| Kraken ping | 30s | SDK responds with pong automatically |
| SDK heartbeat | Configurable (default 30s) | Sends ping, expects pong within `timeout` |
| No response | After `timeout` | Connection marked stale, reconnect triggered |
| Kraken `systemStatus` | On connect | Logged, `on_connection_state_change` fired |

**Kraken endpoints:**
- Public: `wss://ws.kraken.com` - No auth required, ticker/trade/book/ohlc
- Private: `wss://ws-auth.kraken.com` - Requires API key, ownTrades/openOrders

### Reconnection Behavior

| Event | SDK Behavior | User Action Required |
|-------|--------------|---------------------|
| Network disconnect | Auto-reconnect with exponential backoff (100ms → 30s) | None |
| Server close (1000) | Reconnect after `initial_delay` | None |
| Auth failure | Stop reconnecting, emit error | Re-authenticate |
| Max attempts reached | Emit `ConnectionState::Failed` | Manual `connect()` |

**On successful reconnect:**
1. All previous subscriptions are automatically restored
2. `on_connection_state_change(Connected)` fires
3. Order book state is **invalidated** - snapshot required before deltas

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

The SDK maintains order book state with these invariants:

1. **Snapshot first**: Always wait for snapshot before applying deltas
2. **Checksum validation**: Verify CRC32 checksum after each update (Kraken provides this)
3. **Sequence ordering**: Deltas must be applied in sequence order
4. **Stale detection**: Discard deltas older than current snapshot sequence

```
State Machine:
┌─────────────┐    snapshot    ┌──────────────┐    delta     ┌─────────────┐
│ Disconnected│───────────────▶│ Snapshot Rcvd│─────────────▶│ Applying    │
└─────────────┘                └──────────────┘              │ Deltas      │
       ▲                              ▲                      └──────┬──────┘
       │                              │                             │
       │         checksum fail        │      checksum ok            │
       └──────────────────────────────┴─────────────────────────────┘
```

**During resync:** Consumer receives `BookState::Resyncing`. Do not use stale book data.

### Timestamp Guarantees

| Guarantee | Level | Notes |
|-----------|-------|-------|
| Exchange timestamps | **Monotonic per symbol** | Kraken guarantees this |
| Receive timestamps | **Best effort** | Network jitter possible |
| Latency calculation | `receive_time - exchange_time` | Requires NTP sync |

**Clock skew:** If your system clock drifts >1s from exchange, latency metrics will be inaccurate.

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

## Versioning & Compatibility

This SDK follows [Semantic Versioning](https://semver.org/) with a strong compatibility promise:

**Post-1.0 Guarantee:**
- No breaking changes in `prelude` module without major version bump
- Config additions are non-breaking (new fields have defaults)
- New enum variants are non-breaking (`#[non_exhaustive]`)
- MSRV bumps require major version

**Pre-1.0 (current):**
- Minor versions may include breaking changes (documented with `BREAKING:`)
- Patch versions are always safe to upgrade

See [CHANGELOG.md](CHANGELOG.md) for detailed upgrade notes.

---

## Disclaimer

This SDK is not officially affiliated with Kraken. Use at your own risk in production environments.

## Support

For questions and support:

- Check the [examples](examples/) directory
- Review the API documentation: `cargo doc --open`
- Open an issue on GitHub
- See [CHANGELOG.md](CHANGELOG.md) for version history

---

Built with ❤️ in Rust