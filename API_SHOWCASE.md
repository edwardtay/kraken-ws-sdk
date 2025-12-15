# 🦑 Kraken WebSocket SDK - API Showcase

> **This is an SDK, not just a demo.** Here's the proof.

---

## ⚡ Rust API (Native)

```rust
use kraken_ws_sdk::prelude::*;

#[tokio::main]
async fn main() {
    // Create SDK instance
    let sdk = KrakenSDK::default();
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // SUBSCRIBE TO TICKER
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    sdk.subscribe_ticker("BTC/USD", |ticker| {
        println!("BTC: ${:.2}", ticker.last_price);
    });
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // SUBSCRIBE TO ORDER BOOK (with depth)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    sdk.subscribe_orderbook("ETH/USD", 10, |book| {
        let spread = book.asks[0].price - book.bids[0].price;
        println!("ETH spread: ${:.4}", spread);
    });
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // UNSUBSCRIBE
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    sdk.unsubscribe("ETH/USD");
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // RECONNECTION HANDLER
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    sdk.on_reconnect(|attempt| {
        println!("Reconnecting... attempt #{}", attempt);
    });
    
    // Connect and run
    sdk.connect().await.unwrap();
}
```

---

## 🌐 JavaScript API (via WASM) - Same SDK, Browser Ready!

> **🦀 One Codebase. Multiple Targets. Production-Grade Everywhere.**
> 
> The same Rust SDK that powers backend trading bots compiles to WASM
> and runs in your browser. No separate JavaScript implementation.

```javascript
import { KrakenWasm, JsConfig, formatLatency } from 'kraken-ws-sdk';

// Configure (same options as Rust!)
const config = new JsConfig()
    .setAutoReconnect(true)
    .setMaxMessagesPerSecond(500);

const sdk = new KrakenWasm(config);

// Subscribe to ticker
sdk.subscribeTicker("BTC/USD", (ticker) => {
    console.log(`BTC: $${ticker.last_price} | Spread: ${ticker.spread}`);
});

// Subscribe to order book
sdk.subscribeOrderBook("ETH/USD", 10, (book) => {
    console.log(`ETH spread: $${book.asks[0].price - book.bids[0].price}`);
});

// Latency & backpressure (same as Rust!)
const stats = sdk.getBackpressureStats();
console.log(`Dropped: ${stats.total_dropped}`);

// Connect
await sdk.connect();
console.log(sdk.info()); // "Same SDK powers backend bots & frontend UI"
```

### Build WASM

```bash
cargo install wasm-pack
wasm-pack build --target web --features wasm
```

### Use in Browser

```html
<script type="module">
    import init, { KrakenWasm } from './pkg/kraken_ws_sdk.js';
    await init();
    const sdk = new KrakenWasm();
    await sdk.connect();
</script>
```

---

## 📦 API Surface

| Method | Rust | JavaScript | Description |
|--------|------|------------|-------------|
| `subscribe_ticker(pair, callback)` | ✅ | ✅ | Real-time price updates |
| `subscribe_orderbook(pair, depth, callback)` | ✅ | ✅ | Order book with depth |
| `subscribe_trades(pair, callback)` | ✅ | ✅ | Trade stream |
| `unsubscribe(pair)` | ✅ | ✅ | Stop receiving updates |
| `on_reconnect(handler)` | ✅ | ✅ | Handle reconnection |
| `on_error(handler)` | ✅ | ✅ | Handle errors |
| `connect()` | ✅ | ✅ | Connect to Kraken |
| `disconnect()` | ✅ | ✅ | Disconnect |
| `is_connected()` | ✅ | ✅ | Check status |
| `subscribed_pairs()` | ✅ | ✅ | List active subs |

---

## 🔧 Builder Pattern (Advanced Config)

```rust
let sdk = KrakenSDKBuilder::new()
    .endpoint("wss://ws.kraken.com")
    .auto_reconnect(true)
    .max_reconnect_attempts(10)
    .build();
```

---

## 📊 Data Types

```rust
// Ticker
struct TickerData {
    symbol: String,      // "BTC/USD"
    bid: Decimal,        // Best bid price
    ask: Decimal,        // Best ask price
    last_price: Decimal, // Last trade price
    volume: Decimal,     // 24h volume
    timestamp: DateTime<Utc>,
}

// Order Book
struct OrderBookUpdate {
    symbol: String,
    bids: Vec<PriceLevel>,  // Sorted by price desc
    asks: Vec<PriceLevel>,  // Sorted by price asc
    timestamp: DateTime<Utc>,
}

// Trade
struct TradeData {
    symbol: String,
    price: Decimal,
    volume: Decimal,
    side: TradeSide,  // Buy | Sell
    timestamp: DateTime<Utc>,
}
```

---

## 🚀 Why This is an SDK

| Feature | Raw WebSocket | This SDK |
|---------|---------------|----------|
| Type Safety | ❌ JSON parsing | ✅ Strong types |
| Auto Reconnect | ❌ Manual | ✅ Built-in |
| Per-Pair Callbacks | ❌ Single handler | ✅ Multiple |
| Order Book State | ❌ Manual tracking | ✅ Managed |
| Error Handling | ❌ Try/catch | ✅ Structured |
| WASM Support | ❌ N/A | ✅ Browser ready |
| Chainable API | ❌ N/A | ✅ Fluent |

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     KrakenSDK (Public API)                  │
├─────────────────────────────────────────────────────────────┤
│  subscribe_ticker()  │  subscribe_orderbook()  │  connect() │
└──────────┬───────────┴──────────────┬──────────┴────────────┘
           │                          │
           ▼                          ▼
┌─────────────────────┐    ┌─────────────────────┐
│  Event Dispatcher   │    │ Connection Manager  │
│  (Thread-safe)      │    │ (Auto-reconnect)    │
└──────────┬──────────┘    └──────────┬──────────┘
           │                          │
           ▼                          ▼
┌─────────────────────┐    ┌─────────────────────┐
│   Message Parser    │◄───│  WebSocket Client   │
│   (Kraken format)   │    │  (tokio-tungstenite)│
└─────────────────────┘    └─────────────────────┘
```

---

---

## 🔢 Deterministic Message Sequencing (Production-Grade)

```rust
use kraken_ws_sdk::{SequenceManager, SequenceConfig};

// Configure sequence validation
let config = SequenceConfig {
    max_gap_size: 10,           // Resync if gap > 10
    max_pending_messages: 100,  // Resync if too many pending
    auto_resync: true,          // Auto-resync on large gaps
    ..Default::default()
};

let seq_manager = SequenceManager::with_config(config);

// Gap detection callback
seq_manager.on_gap(|event| {
    println!("⚠️ GAP: expected={}, got={}, size={}", 
        event.expected_sequence, 
        event.received_sequence,
        event.gap_size);
});

// Resync callback
seq_manager.on_resync(|event| {
    println!("🔄 RESYNC: channel={}, reason={:?}", 
        event.channel, 
        event.reason);
});

// Validate each message
let result = seq_manager.validate("BTC/USD", sequence, &data);

// Check result
println!("last_sequence: {}", result.state.last_sequence);
println!("gap_detected: {}", result.state.gap_detected);
println!("resync_triggered: {}", result.resync_triggered);
```

### Sequence State

```rust
struct SequenceState {
    last_sequence: u64,      // Last processed sequence
    gap_detected: bool,      // Gap currently detected
    resync_triggered: bool,  // Resync was triggered
    total_gaps: u64,         // Total gaps since start
    messages_processed: u64, // Total messages processed
}
```

### Why This Matters for Trading

| Feature | Benefit |
|---------|---------|
| Gap Detection | Never miss a trade |
| Auto-Resync | Recover from network issues |
| Pending Queue | Handle out-of-order delivery |
| Per-Channel State | Independent tracking |
| Statistics | Monitor data quality |

---

---

## ⚡ Backpressure & Throttling Control (Exchange-Grade)

```rust
use kraken_ws_sdk::{BackpressureManager, BackpressureConfig, DropPolicy};

// Configure flow control
let config = BackpressureConfig {
    max_messages_per_second: 1000,  // Rate limit
    max_buffer_size: 10000,         // Buffer limit
    drop_policy: DropPolicy::Oldest, // What to drop
    coalesce_updates: true,         // Merge same-symbol updates
    burst_allowance: 100,           // Allow short bursts
    ..Default::default()
};

let bp = BackpressureManager::with_config(config);

// Callbacks for monitoring
bp.on_drop(|event| {
    println!("🗑️ DROPPED: {} - {:?}", event.symbol, event.reason);
});

bp.on_coalesce(|event| {
    println!("🔀 COALESCED: {} (seq {} -> {})", 
        event.symbol, event.old_sequence, event.new_sequence);
});

bp.on_rate_limit(|event| {
    println!("⚠️ RATE LIMIT: {:.1} msg/s", event.current_rate);
});

// Process messages
let result = bp.process(message);

// Check result
println!("accepted: {}", result.accepted);
println!("dropped: {}", result.dropped);
println!("coalesced: {}", result.coalesced);
println!("queue_depth: {}", result.queue_depth);
println!("current_rate: {:.1}/s", result.current_rate);
```

### Drop Policies

| Policy | Behavior | Use Case |
|--------|----------|----------|
| `Oldest` | Drop oldest messages first | Keep latest data |
| `Latest` | Reject new messages | Preserve history |
| `Random` | Drop randomly | Statistical fairness |
| `Block` | Never drop, block | Critical data |

### Backpressure Stats

```rust
struct BackpressureStats {
    total_received: u64,     // All messages
    total_accepted: u64,     // Processed messages
    total_dropped: u64,      // Dropped messages
    total_coalesced: u64,    // Merged messages
    peak_rate: f64,          // Peak msg/sec
    current_rate: f64,       // Current msg/sec
    drop_rate: f64,          // Drop percentage
    coalesce_rate: f64,      // Coalesce percentage
}
```

### Why This Matters

| Feature | Benefit |
|---------|---------|
| Rate Limiting | Prevent downstream overload |
| Coalescing | Reduce redundant updates |
| Drop Policies | Graceful degradation |
| Statistics | Monitor system health |
| Callbacks | Real-time alerting |

---

---

## ⏱️ Latency as First-Class Metric (Production-Grade)

```rust
use kraken_ws_sdk::{LatencyTracker, LatencyConfig, LatencyAlertType, format_latency};

// Configure latency tracking
let config = LatencyConfig {
    max_samples: 10000,        // Rolling window size
    histogram_bucket_us: 1000, // 1ms buckets
    histogram_buckets: 100,    // Up to 100ms
    rate_window_secs: 10,      // For samples/sec calc
};

let tracker = LatencyTracker::with_config(config);

// Set alert thresholds (microseconds)
tracker.set_thresholds(
    50_000,   // 50ms network threshold
    5_000,    // 5ms processing threshold
    60_000,   // 60ms total threshold
);

// Alert callback
tracker.on_alert(|alert| {
    match alert.alert_type {
        LatencyAlertType::HighNetworkLatency => println!("🌐 Network slow!"),
        LatencyAlertType::HighTotalLatency => println!("🚨 High latency!"),
        _ => {}
    }
    println!("Latency: {}µs (threshold: {}µs)", 
        alert.latency_us, alert.threshold_us);
});

// Record measurement (exchange timestamp from Kraken)
let measurement = tracker.record(exchange_timestamp, "ticker", "BTC/USD");

println!("Network latency:    {}", format_latency(measurement.network_latency_us as f64));
println!("Processing latency: {}", format_latency(measurement.processing_latency_us as f64));
println!("Total latency:      {}", format_latency(measurement.total_latency_us as f64));

// Get comprehensive statistics
let stats = tracker.stats();

println!("p50:  {}", format_latency(stats.total.p50));
println!("p95:  {}", format_latency(stats.total.p95));
println!("p99:  {}", format_latency(stats.total.p99));
println!("p999: {}", format_latency(stats.total.p999));
println!("Max:  {}", format_latency(stats.total.max));
```

### Latency Measurement

```rust
struct LatencyMeasurement {
    exchange_timestamp: DateTime<Utc>,  // When Kraken sent it
    receive_timestamp: DateTime<Utc>,   // When SDK received it
    process_timestamp: DateTime<Utc>,   // When processing finished
    network_latency_us: i64,            // Exchange → SDK
    processing_latency_us: i64,         // SDK internal
    total_latency_us: i64,              // End-to-end
    channel: String,                    // "ticker", "book", etc.
    symbol: String,                     // "BTC/USD"
}
```

### Latency Percentiles

```rust
struct LatencyPercentiles {
    p50: f64,   // Median
    p75: f64,
    p90: f64,
    p95: f64,
    p99: f64,
    p999: f64,  // Three nines
    min: f64,
    max: f64,
    mean: f64,
    stddev: f64,
}
```

### Latency Histogram

```rust
struct LatencyHistogram {
    buckets: Vec<HistogramBucket>,  // Distribution buckets
    total_samples: u64,
    bucket_width_us: i64,           // Bucket size
}

struct HistogramBucket {
    range_start_us: i64,
    range_end_us: i64,
    count: u64,
    percentage: f64,
}
```

### Why This Matters for Trading

| Feature | Benefit |
|---------|---------|
| Exchange Timestamps | True network latency measurement |
| Percentiles (p95/p99) | Tail latency visibility |
| Histogram | Distribution analysis |
| Alerts | Real-time degradation detection |
| Rolling Window | Memory-efficient tracking |

---

## 🔄 Multi-Exchange Abstraction (Production Architecture)

```rust
use kraken_ws_sdk::{
    Exchange, ExchangeAdapter, ExchangeManager, ExchangeStatus,
    KrakenAdapter, BinanceAdapter, create_adapter,
};

// Create adapters for multiple exchanges
let mut manager = ExchangeManager::new();
manager.add_exchange(create_adapter(Exchange::Kraken));
manager.add_exchange(create_adapter(Exchange::Binance));  // Stub
manager.add_exchange(create_adapter(Exchange::Coinbase)); // Stub

// Connect to all exchanges
let results = manager.connect_all().await;

// Unified subscription API
if let Some(kraken) = manager.get_mut(Exchange::Kraken) {
    kraken.on_ticker(Arc::new(|exchange, ticker| {
        println!("[{:?}] {} @ ${}", exchange, ticker.symbol, ticker.last_price);
    }));
    
    kraken.subscribe_ticker(&"BTC/USD".to_string()).await?;
    kraken.subscribe_orderbook(&"ETH/USD".to_string(), 10).await?;
}

// Check status across all exchanges
for (exchange, status) in manager.status_all() {
    println!("{:?}: {:?}", exchange, status);
}
```

### ExchangeAdapter Trait

```rust
#[async_trait]
pub trait ExchangeAdapter: Send + Sync {
    fn exchange(&self) -> Exchange;
    fn capabilities(&self) -> ExchangeCapabilities;
    fn status(&self) -> ExchangeStatus;
    
    async fn connect(&mut self) -> Result<(), SdkError>;
    async fn disconnect(&mut self) -> Result<(), SdkError>;
    
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> Result<(), SdkError>;
    async fn subscribe_trades(&mut self, symbol: &Symbol) -> Result<(), SdkError>;
    async fn subscribe_orderbook(&mut self, symbol: &Symbol, depth: u32) -> Result<(), SdkError>;
    async fn unsubscribe(&mut self, symbol: &Symbol) -> Result<(), SdkError>;
    
    fn on_ticker(&mut self, callback: TickerCallback);
    fn on_trade(&mut self, callback: TradeCallback);
    fn on_orderbook(&mut self, callback: OrderBookCallback);
}
```

### Exchange Capabilities

| Exchange | Ticker | Trades | OrderBook | OHLC | Rate Limit | Status |
|----------|--------|--------|-----------|------|------------|--------|
| Kraken   | ✅     | ✅     | ✅        | ✅   | 60/s       | Live   |
| Binance  | ✅     | ✅     | ✅        | ✅   | 1200/s     | Stub   |
| Coinbase | ✅     | ✅     | ✅        | ❌   | 100/s      | Stub   |

### Symbol Normalization

```rust
// All exchanges use normalized symbols: "BTC/USD"
// Adapters handle conversion internally:
//   Kraken:   "BTC/USD" → "XBT/USD"
//   Binance:  "BTC/USD" → "BTCUSDT"
//   Coinbase: "BTC/USD" → "BTC-USD"
```

### Why This Matters

| Feature | Benefit |
|---------|---------|
| Unified API | Same code for all exchanges |
| Symbol Normalization | No exchange-specific logic |
| Capability Discovery | Runtime feature detection |
| Centralized Management | Single point of control |
| Easy Extension | Just implement the trait |

---

**This is a real SDK with a clean, minimal API surface.**