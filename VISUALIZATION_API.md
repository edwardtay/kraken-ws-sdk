# Order Book Visualization API

**Status**: ✅ Complete - SDK now supports all features for professional order book visualizers

## Overview

The `kraken-ws-sdk` now provides a complete visualization API enabling third-party developers to build professional-grade order book interfaces. All features are production-ready and included in the SDK core.

---

## Feature Support Summary

### Tier 1 — Core Features (Must-Have)

| Feature | Status | API |
|---------|--------|-----|
| ✅ Real-time updates | **READY** | Native WebSocket streaming, <100ms latency |
| ✅ Bid/ask depth ladder | **READY** | `order_book.get_depth_ladder(n)` |
| ✅ Price grouping (aggregation) | **READY** | `order_book.aggregate(tick_size)` |
| ✅ Mid-price + spread display | **READY** | `order_book.get_mid_price()`, `get_spread()` |
| ✅ Cumulative depth visualization | **READY** | Included in `DepthLadder.cumulative_volume` |
| ✅ Best bid/ask highlight | **READY** | `ladder.best_bid`, `ladder.best_ask` |

### Tier 2 — High Value Features

| Feature | Status | API |
|---------|--------|-----|
| ✅ Order flow highlighting | **READY** | `OrderFlowTracker` with event callbacks |
| ✅ Liquidity imbalance indicator | **READY** | `order_book.get_imbalance_ratio(depth)` |
| ✅ Recent trades overlay | **READY** | `TradesByPriceLevel` with price-aligned trades |
| ✅ Latency indicator | **READY** | `LatencyTracker` with p50/p95/p99 |
| ✅ Market pause detection | **READY** | `MarketHealthTracker` with stale/halt detection |

---

## Quick Start

```rust
use kraken_ws_sdk::visualization::*;
use std::str::FromStr;
use rust_decimal::Decimal;

// 1. Set up trackers
let flow_tracker = OrderFlowTracker::new();
let trade_tracker = TradesByPriceLevel::new();
let health_tracker = MarketHealthTracker::new();

// 2. On each order book update:
let ladder = order_book.get_depth_ladder(20);
let flow_events = flow_tracker.track_update(&order_book);
let imbalance = order_book.get_imbalance_ratio(10);
health_tracker.record_update(&order_book.symbol);

// 3. On each trade:
trade_tracker.add_trade(&trade_data);

// 4. Render your UI with this data
```

---

## New Data Structures

### `DepthLadder`

Complete depth ladder with cumulative sizes and percentages:

```rust
pub struct DepthLadder {
    pub bids: Vec<LadderLevel>,  // Best bid first
    pub asks: Vec<LadderLevel>,  // Best ask first
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub mid_price: Option<Decimal>,
    pub spread: Option<Decimal>,
    pub spread_bps: Option<Decimal>,  // Basis points
}

pub struct LadderLevel {
    pub price: Decimal,
    pub volume: Decimal,
    pub cumulative_volume: Decimal,      // Running total from best
    pub volume_percent: Decimal,         // % of total side volume
    pub cumulative_percent: Decimal,     // % cumulative
    pub distance_from_mid: Option<Decimal>,
    pub distance_bps: Option<Decimal>,
}
```

**Usage**:
```rust
let ladder = order_book.get_depth_ladder(20);

// Render horizontal bars
for level in ladder.bids {
    let bar_width = level.cumulative_percent; // 0-100%
    render_bar(level.price, bar_width, level.volume);
}
```

---

### `AggregatedBook`

Price-aggregated order book (tick size grouping):

```rust
pub struct AggregatedBook {
    pub bids: Vec<AggregatedLevel>,
    pub asks: Vec<AggregatedLevel>,
    pub tick_size: Decimal,
}

pub struct AggregatedLevel {
    pub price: Decimal,           // Bucket price
    pub volume: Decimal,          // Total volume in bucket
    pub order_count: usize,       // Orders aggregated
}
```

**Usage**:
```rust
// Group BTC into $100 buckets
let aggregated = order_book.aggregate(Decimal::from(100));

// Reduces noise from $94,123.45, $94,124.32, etc.
// to clean $94,000, $94,100, $94,200 levels
```

---

### `ImbalanceMetrics`

Liquidity imbalance analysis:

```rust
pub struct ImbalanceMetrics {
    pub imbalance_ratio: Decimal,  // -1.0 (all ask) to +1.0 (all bid)
    pub bid_volume: Decimal,
    pub ask_volume: Decimal,
    pub bid_vwap: Option<Decimal>, // Volume-weighted average price
    pub ask_vwap: Option<Decimal>,
    pub depth_levels: usize,
}
```

**Usage**:
```rust
let metrics = order_book.get_imbalance_metrics(10);

if metrics.imbalance_ratio > Decimal::from_str("0.3").unwrap() {
    show_indicator("buy_pressure");
}
```

---

### `FlowEvent`

Order flow changes (large order detection):

```rust
pub enum FlowEventType {
    LargeOrderAppeared,
    LargeOrderDisappeared,
    SizeIncreased { delta: Decimal },
    SizeDecreased { delta: Decimal },
    BestBidChanged { old: Decimal, new: Decimal },
    BestAskChanged { old: Decimal, new: Decimal },
}

pub struct FlowEvent {
    pub price: Decimal,
    pub side: FlowSide,           // Bid or Ask
    pub event_type: FlowEventType,
    pub current_volume: Decimal,
    pub previous_volume: Decimal,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}
```

**Usage**:
```rust
let mut tracker = OrderFlowTracker::with_config(OrderFlowConfig {
    large_order_threshold: Decimal::from(10), // 10 BTC
    ..Default::default()
});

tracker.on_event(|event| {
    match event.event_type {
        FlowEventType::LargeOrderAppeared => {
            flash_animation(event.price, "green");
        }
        FlowEventType::LargeOrderDisappeared => {
            flash_animation(event.price, "red");
        }
        _ => {}
    }
});

// On each update
let events = tracker.track_update(&order_book);
```

---

### `LevelTradeStats`

Recent trades aligned with price levels:

```rust
pub struct LevelTradeStats {
    pub price: Decimal,
    pub total_volume: Decimal,
    pub trade_count: usize,
    pub buy_volume: Decimal,
    pub sell_volume: Decimal,
    pub last_trade: Option<LevelTrade>,
    pub avg_trade_size: Decimal,
}

pub struct LevelTrade {
    pub price: Decimal,
    pub volume: Decimal,
    pub side: TradeSide,
    pub age_ms: u64,  // For fade effects
}
```

**Usage**:
```rust
let tracker = TradesByPriceLevel::new();

// Add trades
tracker.add_trade(&trade_data);

// Get overlay for visualization
let overlay = tracker.get_trade_overlay("BTC/USD");
for stats in overlay {
    render_trade_indicator(stats.price, stats.trade_count, stats.buy_volume);
}
```

---

### `MarketStatus`

Market health / stale detection:

```rust
pub enum MarketStatus {
    Active,      // Receiving updates
    Stale,       // No updates for 5+ seconds
    Halted,      // No updates for 30+ seconds
    Unknown,     // No data yet
}
```

**Usage**:
```rust
let health = MarketHealthTracker::new();

health.record_update("BTC/USD");

match health.check_status("BTC/USD") {
    MarketStatus::Active => show_indicator("green"),
    MarketStatus::Stale => show_indicator("yellow"),
    MarketStatus::Halted => show_indicator("red"),
    _ => {}
}
```

---

## Complete Example: Professional Order Book UI

```rust
use kraken_ws_sdk::{KrakenWsClient, ClientConfig, Channel, SdkEvent};
use kraken_ws_sdk::visualization::*;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up SDK client
    let mut client = KrakenWsClient::new(ClientConfig::default());
    let mut events = client.events();
    
    client.subscribe(vec![
        Channel::new("book").with_symbol("BTC/USD"),
        Channel::new("trade").with_symbol("BTC/USD"),
    ]).await?;
    client.connect().await?;
    
    // 2. Set up visualization trackers
    let flow_tracker = OrderFlowTracker::with_config(OrderFlowConfig {
        large_order_threshold: Decimal::from(10),
        track_depth: 25,
        ..Default::default()
    });
    
    let trade_tracker = TradesByPriceLevel::new();
    let health_tracker = MarketHealthTracker::new();
    let latency_tracker = LatencyTracker::new();
    
    // 3. Register callbacks for animations
    flow_tracker.on_event(|event| {
        match event.event_type {
            FlowEventType::LargeOrderAppeared => {
                // Flash green at this price level
                println!("⬆️ Large order at {}", event.price);
            }
            FlowEventType::LargeOrderDisappeared => {
                // Flash red
                println!("⬇️ Order removed from {}", event.price);
            }
            _ => {}
        }
    });
    
    // 4. Main event loop
    while let Some(event) = events.recv().await {
        match event {
            SdkEvent::OrderBook(update) => {
                // Track health
                health_tracker.record_update(&update.symbol);
                
                // Build order book
                let book_manager = OrderBookManager::new();
                let book = book_manager.apply_update(update)?;
                
                // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                // Get visualization data
                // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                
                // Depth ladder with cumulative sizes
                let ladder = book.get_depth_ladder(20);
                
                // Aggregated view (reduce noise)
                let aggregated = book.aggregate(Decimal::from(100));
                
                // Imbalance indicator
                let imbalance = book.get_imbalance_ratio(10);
                let pressure = book.get_book_pressure(10);
                
                // Order flow events
                let flow_events = flow_tracker.track_update(&book);
                
                // Trade overlay
                let trade_overlay = trade_tracker.get_trade_overlay(&book.symbol);
                
                // Market status
                let status = health_tracker.check_status(&book.symbol);
                
                // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                // Render UI
                // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                
                println!("\n═══ BTC/USD Order Book ═══");
                println!("Status: {:?} | Imbalance: {:.2}% | Signal: {:?}", 
                    status, 
                    imbalance * Decimal::from(100),
                    pressure.signal
                );
                
                println!("\n   PRICE    │  SIZE  │ CUM SIZE │  %  │ TRADES");
                println!("──────────────────────────────────────────────────");
                
                // Render asks (top to bottom)
                for level in ladder.asks.iter().rev().take(10) {
                    let trades = trade_overlay.iter()
                        .find(|t| t.price == level.price)
                        .map(|t| t.trade_count)
                        .unwrap_or(0);
                    
                    println!("{:>10} │ {:>6.2} │ {:>8.2} │ {:>4.1}% │ {}", 
                        level.price,
                        level.volume,
                        level.cumulative_volume,
                        level.cumulative_percent,
                        if trades > 0 { format!("{} 🔴", trades) } else { "".to_string() }
                    );
                }
                
                println!("━━━━━━━━━━━ SPREAD: {:?} ━━━━━━━━━━━", ladder.spread);
                
                // Render bids (top to bottom)
                for level in ladder.bids.iter().take(10) {
                    let trades = trade_overlay.iter()
                        .find(|t| t.price == level.price)
                        .map(|t| t.trade_count)
                        .unwrap_or(0);
                    
                    println!("{:>10} │ {:>6.2} │ {:>8.2} │ {:>4.1}% │ {}", 
                        level.price,
                        level.volume,
                        level.cumulative_volume,
                        level.cumulative_percent,
                        if trades > 0 { format!("{} 🟢", trades) } else { "".to_string() }
                    );
                }
            }
            
            SdkEvent::Trade(trade) => {
                // Add to trade overlay
                trade_tracker.add_trade(&trade);
                
                println!("💰 Trade: {} {} @ {}", 
                    trade.volume, 
                    match trade.side {
                        TradeSide::Buy => "🟢 BUY",
                        TradeSide::Sell => "🔴 SELL"
                    },
                    trade.price
                );
            }
            
            _ => {}
        }
    }
    
    Ok(())
}
```

---

## Module Organization

```
kraken-ws-sdk/
├── src/
│   ├── orderbook.rs       ← Enhanced with visualization methods
│   ├── orderflow.rs       ← NEW: Flow tracking + trades overlay
│   └── lib.rs             ← Exports `visualization` module
│
└── Exports:
    ├── kraken_ws_sdk::visualization::*  ← All-in-one for builders
    └── kraken_ws_sdk::extended::*       ← Advanced features
```

---

## API Guarantees

All visualization APIs are in the `extended` module with stability guarantees:

- ✅ **Stable**: Won't break between minor versions (same as `prelude`)
- ✅ **Tested**: Full unit test coverage
- ✅ **Production-ready**: Used in SDK web demo
- ✅ **Zero dependencies**: Built on top of existing SDK infrastructure

---

## Next Steps for Third-Party Developers

### 1. Use the `visualization` module:
```rust
use kraken_ws_sdk::visualization::*;
```

### 2. Implement your UI with the provided data structures:
- `DepthLadder` → depth chart / ladder view
- `AggregatedBook` → tick-aggregated view
- `FlowEvent` → flash animations
- `LevelTradeStats` → trade tape overlay
- `MarketStatus` → health indicator

### 3. Reference the complete example above

---

## Features by Use Case

### Building a Depth Chart?
Use: `get_depth_ladder()` + `cumulative_volume` + `cumulative_percent`

### Building a Ladder/DOM?
Use: `get_depth_ladder()` + `FlowEvent` callbacks for animations

### Need Liquidity Heatmap?
Use: `get_imbalance_metrics()` over multiple depth levels

### Want to Show Recent Trades at Levels?
Use: `TradesByPriceLevel` + `get_trade_overlay()`

### Need Trading Signals?
Use: `get_book_pressure()` → `PressureSignal` enum

### Reduce Noise on High-Frequency Markets?
Use: `aggregate(tick_size)` to group price levels

---

## Performance Notes

- **Zero-copy where possible**: BTreeMap traversal, no unnecessary clones
- **Configurable depth**: Track only N levels you need
- **Event-driven**: Callbacks fire only on changes
- **Efficient aggregation**: O(n) for aggregation, O(log n) for lookups
- **Memory-bounded**: Ring buffers for history (configurable limits)

---

## Support

All features are documented in the main SDK README and inline docs:

```bash
cargo doc --open --features full
```

Questions? Check:
- `README.md` → Main documentation
- `examples/` → Working examples
- `src/orderbook.rs` → API docs
- `src/orderflow.rs` → Flow tracking docs

---

**Status**: ✅ Complete and ready for third-party developers to build professional order book visualizers.


