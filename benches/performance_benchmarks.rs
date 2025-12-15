//! Performance benchmarks for the Kraken WebSocket SDK

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kraken_ws_sdk::{
    parser::{KrakenDataParser, DataParser, MessageHandler},
    events::EventDispatcher,
    orderbook::{OrderBookManager, OrderBook},
    data::*,
};
use std::sync::Arc;
use rust_decimal::Decimal;
use std::str::FromStr;
use chrono::Utc;

// Benchmark message parsing
fn bench_parse_ticker(c: &mut Criterion) {
    let parser = KrakenDataParser::new();
    
    let ticker_message = r#"[
        0,
        {
            "a": ["50001.00000", "1", "1.000"],
            "b": ["50000.00000", "2", "2.000"],
            "c": ["50000.50000", "0.10000000"],
            "v": ["100.00000000", "200.00000000"]
        },
        "ticker",
        "XBT/USD"
    ]"#;
    
    c.bench_function("parse_ticker", |b| {
        b.iter(|| {
            let _ = parser.parse_ticker(black_box(ticker_message));
        })
    });
}

fn bench_parse_trade(c: &mut Criterion) {
    let parser = KrakenDataParser::new();
    
    let trade_message = r#"[
        0,
        [
            ["50000.00000", "0.10000000", "1234567890.123456", "b", "l", ""]
        ],
        "trade",
        "XBT/USD"
    ]"#;
    
    c.bench_function("parse_trade", |b| {
        b.iter(|| {
            let _ = parser.parse_trade(black_box(trade_message));
        })
    });
}

fn bench_parse_orderbook(c: &mut Criterion) {
    let parser = KrakenDataParser::new();
    
    let orderbook_message = r#"[
        0,
        {
            "b": [
                ["50000.00000", "1.00000000", "1234567890.123456"],
                ["49999.00000", "2.00000000", "1234567890.123456"]
            ],
            "a": [
                ["50001.00000", "0.50000000", "1234567890.123456"],
                ["50002.00000", "1.50000000", "1234567890.123456"]
            ]
        },
        "book-10",
        "XBT/USD"
    ]"#;
    
    c.bench_function("parse_orderbook", |b| {
        b.iter(|| {
            let _ = parser.parse_orderbook(black_box(orderbook_message));
        })
    });
}

// Benchmark message handling
fn bench_message_handler(c: &mut Criterion) {
    let parser: Arc<dyn DataParser> = Arc::new(KrakenDataParser::new());
    let dispatcher = Arc::new(EventDispatcher::new());
    let handler = MessageHandler::new(parser, dispatcher);
    
    let messages = vec![
        r#"{"event":"heartbeat"}"#,
        r#"{"event":"systemStatus","status":"online"}"#,
        r#"{"event":"subscriptionStatus","status":"subscribed"}"#,
    ];
    
    c.bench_function("handle_system_messages", |b| {
        b.iter(|| {
            for message in &messages {
                let _ = tokio_test::block_on(handler.handle_message(black_box(message)));
            }
        })
    });
}

// Benchmark order book operations
fn bench_orderbook_updates(c: &mut Criterion) {
    let manager = OrderBookManager::new();
    
    let update = OrderBookUpdate {
        symbol: "BTC/USD".to_string(),
        bids: vec![
            PriceLevel {
                price: Decimal::from_str("50000.0").unwrap(),
                volume: Decimal::from_str("1.5").unwrap(),
                timestamp: Utc::now(),
            },
        ],
        asks: vec![
            PriceLevel {
                price: Decimal::from_str("50001.0").unwrap(),
                volume: Decimal::from_str("1.0").unwrap(),
                timestamp: Utc::now(),
            },
        ],
        timestamp: Utc::now(),
        checksum: None,
    };
    
    c.bench_function("orderbook_update", |b| {
        b.iter(|| {
            let _ = manager.apply_update(black_box(update.clone()));
        })
    });
}

fn bench_orderbook_calculations(c: &mut Criterion) {
    let mut order_book = OrderBook::new("BTC/USD");
    
    // Add multiple price levels
    for i in 0..100 {
        let bid_price = Decimal::from_str(&format!("{}", 50000 - i)).unwrap();
        let ask_price = Decimal::from_str(&format!("{}", 50001 + i)).unwrap();
        
        order_book.bids.insert(
            bid_price,
            PriceLevel {
                price: bid_price,
                volume: Decimal::from_str("1.0").unwrap(),
                timestamp: Utc::now(),
            },
        );
        
        order_book.asks.insert(
            ask_price,
            PriceLevel {
                price: ask_price,
                volume: Decimal::from_str("1.0").unwrap(),
                timestamp: Utc::now(),
            },
        );
    }
    
    c.bench_function("orderbook_spread_calculation", |b| {
        b.iter(|| {
            let _ = order_book.get_spread();
        })
    });
    
    c.bench_function("orderbook_mid_price_calculation", |b| {
        b.iter(|| {
            let _ = order_book.get_mid_price();
        })
    });
    
    c.bench_function("orderbook_total_volume_calculation", |b| {
        b.iter(|| {
            let _ = order_book.get_total_volume();
        })
    });
}

// Benchmark event dispatching
fn bench_event_dispatching(c: &mut Criterion) {
    let dispatcher = Arc::new(EventDispatcher::new());
    
    // Create test callback
    struct BenchCallback;
    impl kraken_ws_sdk::EventCallback for BenchCallback {
        fn on_ticker(&self, _data: TickerData) {}
        fn on_orderbook(&self, _data: OrderBookUpdate) {}
        fn on_trade(&self, _data: TradeData) {}
        fn on_ohlc(&self, _data: OHLCData) {}
        fn on_error(&self, _error: kraken_ws_sdk::SdkError) {}
        fn on_connection_state_change(&self, _state: ConnectionState) {}
    }
    
    let callback: Arc<dyn kraken_ws_sdk::EventCallback> = Arc::new(BenchCallback);
    
    // Register multiple callbacks
    for _ in 0..10 {
        dispatcher.register_callback(DataType::Ticker, callback.clone());
    }
    
    let ticker_data = TickerData {
        symbol: "BTC/USD".to_string(),
        bid: Decimal::from_str("50000.0").unwrap(),
        ask: Decimal::from_str("50001.0").unwrap(),
        last_price: Decimal::from_str("50000.5").unwrap(),
        volume: Decimal::from_str("100.0").unwrap(),
        timestamp: Utc::now(),
    };
    
    c.bench_function("dispatch_ticker_to_10_callbacks", |b| {
        b.iter(|| {
            dispatcher.dispatch_ticker(black_box(ticker_data.clone()));
        })
    });
}

// Benchmark with different message sizes
fn bench_message_sizes(c: &mut Criterion) {
    let parser = KrakenDataParser::new();
    
    let mut group = c.benchmark_group("message_parsing_by_size");
    
    // Small message
    let small_message = r#"{"event":"heartbeat"}"#;
    group.bench_with_input(
        BenchmarkId::new("small", small_message.len()),
        &small_message,
        |b, message| {
            b.iter(|| {
                let _ = serde_json::from_str::<serde_json::Value>(black_box(message));
            })
        },
    );
    
    // Medium message (ticker)
    let medium_message = r#"[0,{"a":["50001.00000","1","1.000"],"b":["50000.00000","2","2.000"],"c":["50000.50000","0.10000000"],"v":["100.00000000","200.00000000"]},"ticker","XBT/USD"]"#;
    group.bench_with_input(
        BenchmarkId::new("medium", medium_message.len()),
        &medium_message,
        |b, message| {
            b.iter(|| {
                let _ = serde_json::from_str::<serde_json::Value>(black_box(message));
            })
        },
    );
    
    // Large message (order book with many levels)
    let mut large_bids = String::new();
    let mut large_asks = String::new();
    
    for i in 0..100 {
        if i > 0 {
            large_bids.push(',');
            large_asks.push(',');
        }
        large_bids.push_str(&format!(r#"["{}.00000","1.00000000","1234567890.123456"]"#, 50000 - i));
        large_asks.push_str(&format!(r#"["{}.00000","1.00000000","1234567890.123456"]"#, 50001 + i));
    }
    
    let large_message = format!(
        r#"[0,{{"b":[{}],"a":[{}]}},"book-100","XBT/USD"]"#,
        large_bids, large_asks
    );
    
    group.bench_with_input(
        BenchmarkId::new("large", large_message.len()),
        &large_message,
        |b, message| {
            b.iter(|| {
                let _ = serde_json::from_str::<serde_json::Value>(black_box(message));
            })
        },
    );
    
    group.finish();
}

// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let manager = Arc::new(OrderBookManager::new());
    
    c.bench_function("concurrent_orderbook_reads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    let manager_clone = Arc::clone(&manager);
                    tokio::spawn(async move {
                        let _ = manager_clone.get_order_book("BTC/USD");
                        let _ = manager_clone.get_best_bid_ask("BTC/USD");
                    })
                })
                .collect();
            
            // Wait for all tasks to complete
            for handle in handles {
                let _ = tokio_test::block_on(handle);
            }
        })
    });
}

criterion_group!(
    benches,
    bench_parse_ticker,
    bench_parse_trade,
    bench_parse_orderbook,
    bench_message_handler,
    bench_orderbook_updates,
    bench_orderbook_calculations,
    bench_event_dispatching,
    bench_message_sizes,
    bench_concurrent_operations
);

criterion_main!(benches);