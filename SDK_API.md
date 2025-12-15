# Kraken WebSocket SDK - API Reference

## 🎯 Minimal but Powerful API

The SDK exposes a clean, chainable API for real-time cryptocurrency data.

---

## Rust API

```rust
use kraken_ws_sdk::prelude::*;

#[tokio::main]
async fn main() {
    let sdk = KrakenSDK::default();
    
    // Subscribe to ticker updates
    sdk.subscribe_ticker("BTC/USD", |ticker| {
        println!("BTC: ${} (spread: ${})", 
            ticker.last_price, 
            ticker.ask - ticker.bid);
    });
    
    // Subscribe to order book with depth
    sdk.subscribe_orderbook("ETH/USD", 10, |book| {
        println!("ETH bids: {}, asks: {}", 
            book.bids.len(), 
            book.asks.len());
    });
    
    // Subscribe to trades
    sdk.subscribe_trades("BTC/USD", |trade| {
        println!("{:?} {} @ ${}", 
            trade.side, trade.volume, trade.price);
    });
    
    // Handle reconnection
    sdk.on_reconnect(|attempt| {
        println!("Reconnecting... #{}", attempt);
    });
    
    // Connect
    sdk.connect().await.unwrap();
    
    // Later: unsubscribe
    sdk.unsubscribe("ETH/USD");
    
    // Disconnect
    sdk.disconnect().await.unwrap();
}
```

---

## JavaScript API (via WASM)

```javascript
import { KrakenWasm } from 'kraken-ws-sdk';

const sdk = new KrakenWasm();

// Subscribe to ticker updates
sdk.subscribeTicker("BTC/USD", (ticker) => {
    console.log(`BTC: $${ticker.last_price}`);
});

// Subscribe to order book with depth
sdk.subscribeOrderBook("ETH/USD", 10, (book) => {
    console.log(`Best bid: $${book.bids[0]?.price}`);
});

// Subscribe to trades
sdk.subscribeTrades("BTC/USD", (trade) => {
    console.log(`${trade.side} ${trade.volume} @ $${trade.price}`);
});

// Handle reconnection
sdk.onReconnect((attempt) => {
    console.log(`Reconnecting... attempt #${attempt}`);
});

// Connect
await sdk.connect();

// Unsubscribe
sdk.unsubscribe("ETH/USD");

// Check status
console.log("Connected:", sdk.isConnected());
console.log("Pairs:", sdk.subscribedPairs());

// Disconnect
sdk.disconnect();
```

---

## TypeScript Definitions

```typescript
interface Ticker {
    symbol: string;
    bid: number;
    ask: number;
    last_price: number;
    volume: number;
}

interface OrderBook {
    symbol: string;
    bids: PriceLevel[];
    asks: PriceLevel[];
}

interface PriceLevel {
    price: number;
    volume: number;
}

interface Trade {
    symbol: string;
    price: number;
    volume: number;
    side: 'Buy' | 'Sell';
}

declare class KrakenWasm {
    constructor();
    
    subscribeTicker(pair: string, callback: (ticker: Ticker) => void): void;
    subscribeOrderBook(pair: string, depth: number, callback: (book: OrderBook) => void): void;
    subscribeTrades(pair: string, callback: (trade: Trade) => void): void;
    unsubscribe(pair: string): void;
    onReconnect(handler: (attempt: number) => void): void;
    
    connect(): Promise<void>;
    disconnect(): void;
    isConnected(): boolean;
    subscribedPairs(): string[];
}
```

---

## API Methods

| Method | Description | Rust | JS/WASM |
|--------|-------------|------|---------|
| `subscribe_ticker(pair, callback)` | Subscribe to price updates | ✅ | ✅ |
| `subscribe_orderbook(pair, depth, callback)` | Subscribe to order book | ✅ | ✅ |
| `subscribe_trades(pair, callback)` | Subscribe to trades | ✅ | ✅ |
| `unsubscribe(pair)` | Unsubscribe from pair | ✅ | ✅ |
| `on_reconnect(handler)` | Handle reconnection | ✅ | ✅ |
| `on_error(handler)` | Handle errors | ✅ | ✅ |
| `connect()` | Connect to WebSocket | ✅ | ✅ |
| `disconnect()` | Disconnect | ✅ | ✅ |
| `is_connected()` | Check connection | ✅ | ✅ |
| `subscribed_pairs()` | List subscriptions | ✅ | ✅ |

---

## Supported Trading Pairs

All Kraken trading pairs are supported. Common examples:

- `BTC/USD` - Bitcoin
- `ETH/USD` - Ethereum
- `ADA/USD` - Cardano
- `SOL/USD` - Solana
- `DOT/USD` - Polkadot
- `XRP/USD` - Ripple

---

## Build for WASM

```bash
# Install wasm-pack
cargo install wasm-pack

# Build WASM package
wasm-pack build --target web --features wasm

# Use in browser
<script type="module">
  import init, { KrakenWasm } from './pkg/kraken_ws_sdk.js';
  await init();
  const sdk = new KrakenWasm();
</script>
```

---

## 🔧 SDK Developer Features

### Retry Policies

```rust
use kraken_ws_sdk::prelude::*;

// Configure retry behavior
let policy = RetryPolicy::builder()
    .max_attempts(5)
    .initial_delay(Duration::from_millis(100))
    .max_delay(Duration::from_secs(30))
    .backoff_multiplier(2.0)
    .with_jitter(true)
    .build();

// Pre-built policies
let aggressive = RetryPolicy::aggressive();  // 10 attempts, fast retry
let conservative = RetryPolicy::conservative();  // 2 attempts, slow retry
let none = RetryPolicy::none();  // No retries
```

### Circuit Breaker

```rust
use kraken_ws_sdk::prelude::*;

let mut breaker = CircuitBreaker::new(
    5,  // failure threshold
    Duration::from_secs(30)  // reset timeout
);

if breaker.allow_request() {
    match do_operation().await {
        Ok(_) => breaker.record_success(),
        Err(_) => breaker.record_failure(),
    }
}

// Check state
match breaker.state() {
    CircuitState::Closed => println!("Normal operation"),
    CircuitState::Open => println!("Circuit open - failing fast"),
    CircuitState::HalfOpen => println!("Testing recovery"),
}
```

### Middleware/Interceptors

```rust
use kraken_ws_sdk::prelude::*;

// Build middleware chain
let chain = MiddlewareChain::new()
    .add(LoggingMiddleware::info())
    .add(MetricsMiddleware::new(metrics.clone()))
    .add(RateLimitMiddleware::new(100));  // 100 req/sec

// Execute with middleware
let mut ctx = RequestContext::new("subscribe")
    .with_metadata("pair", "BTC/USD");

chain.execute_before(&mut ctx).await?;
// ... do operation ...
chain.execute_after(&ResponseContext::success(&ctx)).await;
```

### Telemetry & Metrics

```rust
use kraken_ws_sdk::prelude::*;

// Configure telemetry
let config = TelemetryConfig::builder()
    .service_name("my-trading-bot")
    .with_metrics(true)
    .with_tracing(true)
    .label("env", "production")
    .build();

// Create metrics registry
let registry = MetricsRegistry::new(config);
let sdk_metrics = SdkMetrics::new(&registry);

// Record metrics
sdk_metrics.messages_received.inc();
sdk_metrics.message_latency.observe_duration(latency);
sdk_metrics.active_subscriptions.set(5);

// Export Prometheus format
let prometheus_output = registry.export_prometheus();
```

### Latency Tracking

```rust
use kraken_ws_sdk::prelude::*;

let tracker = LatencyTracker::new(LatencyConfig {
    window_size: 1000,
    alert_threshold_ms: 100,
    ..Default::default()
});

// Record measurements
tracker.record(LatencyMeasurement {
    exchange_timestamp: exchange_time,
    receive_timestamp: Utc::now(),
    symbol: "BTC/USD".to_string(),
});

// Get statistics
let stats = tracker.stats();
println!("p50: {}ms, p99: {}ms", stats.p50_ms, stats.p99_ms);
```

### Backpressure Control

```rust
use kraken_ws_sdk::prelude::*;

let config = BackpressureConfig {
    max_messages_per_second: 1000,
    max_buffer_size: 10000,
    drop_policy: DropPolicy::Oldest,
    coalesce_updates: true,
    burst_allowance: 100,
};

let manager = BackpressureManager::new(config);

// Set callbacks
manager.on_drop(|event| {
    println!("Dropped {} messages: {:?}", event.count, event.reason);
});

manager.on_rate_limit(|event| {
    println!("Rate limited: {} msg/s", event.current_rate);
});
```

### Sequence Gap Detection

```rust
use kraken_ws_sdk::prelude::*;

let config = SequenceConfig {
    max_gap_size: 10,
    resync_threshold: 100,
    pending_timeout: Duration::from_secs(5),
};

let manager = SequenceManager::new(config);

manager.on_gap(|event| {
    println!("Gap detected: {} -> {}", event.expected, event.received);
});

manager.on_resync(|event| {
    println!("Resyncing: {:?}", event.reason);
});
```

---

## Why This SDK?

| Feature | This SDK | Raw WebSocket |
|---------|----------|---------------|
| Type Safety | ✅ Full types | ❌ Manual parsing |
| Auto Reconnect | ✅ Built-in | ❌ DIY |
| Error Handling | ✅ Structured errors with codes | ❌ Try/catch |
| Retry Policies | ✅ Configurable with jitter | ❌ Manual |
| Circuit Breaker | ✅ Built-in | ❌ DIY |
| Middleware | ✅ Interceptor chain | ❌ N/A |
| Metrics/Telemetry | ✅ Prometheus export | ❌ Manual |
| Rate Limiting | ✅ Client-side aware | ❌ DIY |
| Backpressure | ✅ Drop policies | ❌ Buffer overflow |
| Sequence Tracking | ✅ Gap detection | ❌ Manual |
| WASM Support | ✅ Browser ready | ❌ N/A |
| Order Book State | ✅ Managed | ❌ Manual |

---

Built with ❤️ in Rust