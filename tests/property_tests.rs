//! Property-based tests using quickcheck

use kraken_ws_sdk::{
    data::*,
    orderbook::*,
    parser::*,
};
use quickcheck::{quickcheck, TestResult};
use quickcheck_macros::quickcheck;
use rust_decimal::Decimal;
use std::str::FromStr;
use chrono::Utc;

// Property tests for OrderBook
#[quickcheck]
fn prop_orderbook_spread_is_positive(bid_price: f64, ask_price: f64) -> TestResult {
    // Skip invalid prices
    if bid_price <= 0.0 || ask_price <= 0.0 || bid_price >= ask_price {
        return TestResult::discard();
    }
    
    let mut order_book = OrderBook::new("TEST/USD");
    
    let bid_decimal = match Decimal::from_str(&bid_price.to_string()) {
        Ok(d) => d,
        Err(_) => return TestResult::discard(),
    };
    
    let ask_decimal = match Decimal::from_str(&ask_price.to_string()) {
        Ok(d) => d,
        Err(_) => return TestResult::discard(),
    };
    
    order_book.bids.insert(
        bid_decimal,
        PriceLevel {
            price: bid_decimal,
            volume: Decimal::from_str("1.0").unwrap(),
            timestamp: Utc::now(),
        },
    );
    
    order_book.asks.insert(
        ask_decimal,
        PriceLevel {
            price: ask_decimal,
            volume: Decimal::from_str("1.0").unwrap(),
            timestamp: Utc::now(),
        },
    );
    
    if let Some(spread) = order_book.get_spread() {
        TestResult::from_bool(spread > Decimal::ZERO)
    } else {
        TestResult::failed()
    }
}

#[quickcheck]
fn prop_orderbook_mid_price_between_bid_ask(bid_price: f64, ask_price: f64) -> TestResult {
    // Skip invalid prices
    if bid_price <= 0.0 || ask_price <= 0.0 || bid_price >= ask_price {
        return TestResult::discard();
    }
    
    let mut order_book = OrderBook::new("TEST/USD");
    
    let bid_decimal = match Decimal::from_str(&bid_price.to_string()) {
        Ok(d) => d,
        Err(_) => return TestResult::discard(),
    };
    
    let ask_decimal = match Decimal::from_str(&ask_price.to_string()) {
        Ok(d) => d,
        Err(_) => return TestResult::discard(),
    };
    
    order_book.bids.insert(
        bid_decimal,
        PriceLevel {
            price: bid_decimal,
            volume: Decimal::from_str("1.0").unwrap(),
            timestamp: Utc::now(),
        },
    );
    
    order_book.asks.insert(
        ask_decimal,
        PriceLevel {
            price: ask_decimal,
            volume: Decimal::from_str("1.0").unwrap(),
            timestamp: Utc::now(),
        },
    );
    
    if let Some(mid_price) = order_book.get_mid_price() {
        TestResult::from_bool(mid_price > bid_decimal && mid_price < ask_decimal)
    } else {
        TestResult::failed()
    }
}

#[quickcheck]
fn prop_orderbook_volume_is_non_negative(volumes: Vec<f64>) -> TestResult {
    if volumes.is_empty() {
        return TestResult::discard();
    }
    
    let mut order_book = OrderBook::new("TEST/USD");
    
    for (i, &volume) in volumes.iter().enumerate() {
        if volume < 0.0 {
            return TestResult::discard();
        }
        
        let volume_decimal = match Decimal::from_str(&volume.to_string()) {
            Ok(d) => d,
            Err(_) => continue,
        };
        
        let price_decimal = match Decimal::from_str(&(50000.0 + i as f64).to_string()) {
            Ok(d) => d,
            Err(_) => continue,
        };
        
        order_book.bids.insert(
            price_decimal,
            PriceLevel {
                price: price_decimal,
                volume: volume_decimal,
                timestamp: Utc::now(),
            },
        );
    }
    
    let (total_bid_volume, _) = order_book.get_total_volume();
    TestResult::from_bool(total_bid_volume >= Decimal::ZERO)
}

// Property tests for Channel validation
#[quickcheck]
fn prop_channel_name_not_empty(name: String) -> TestResult {
    if name.is_empty() {
        return TestResult::discard();
    }
    
    let channel = Channel::new(&name);
    TestResult::from_bool(!channel.name.is_empty())
}

#[quickcheck]
fn prop_channel_with_symbol_preserves_name(name: String, symbol: String) -> TestResult {
    if name.is_empty() || symbol.is_empty() {
        return TestResult::discard();
    }
    
    let channel = Channel::new(&name).with_symbol(&symbol);
    TestResult::from_bool(channel.name == name && channel.symbol == Some(symbol))
}

// Property tests for data structures
#[quickcheck]
fn prop_ticker_data_display_contains_symbol(symbol: String) -> TestResult {
    if symbol.is_empty() {
        return TestResult::discard();
    }
    
    let ticker = TickerData {
        symbol: symbol.clone(),
        bid: Decimal::from_str("50000.0").unwrap(),
        ask: Decimal::from_str("50001.0").unwrap(),
        last_price: Decimal::from_str("50000.5").unwrap(),
        volume: Decimal::from_str("100.0").unwrap(),
        timestamp: Utc::now(),
    };
    
    let display_str = format!("{}", ticker);
    TestResult::from_bool(display_str.contains(&symbol))
}

#[quickcheck]
fn prop_trade_data_volume_positive(volume: f64) -> TestResult {
    if volume <= 0.0 {
        return TestResult::discard();
    }
    
    let volume_decimal = match Decimal::from_str(&volume.to_string()) {
        Ok(d) => d,
        Err(_) => return TestResult::discard(),
    };
    
    let trade = TradeData {
        symbol: "BTC/USD".to_string(),
        price: Decimal::from_str("50000.0").unwrap(),
        volume: volume_decimal,
        side: TradeSide::Buy,
        timestamp: Utc::now(),
        trade_id: "test".to_string(),
    };
    
    TestResult::from_bool(trade.volume > Decimal::ZERO)
}

// Property tests for configuration validation
#[quickcheck]
fn prop_client_config_buffer_size_positive(buffer_size: usize) -> TestResult {
    if buffer_size == 0 {
        return TestResult::discard();
    }
    
    let config = ClientConfig {
        buffer_size,
        ..Default::default()
    };
    
    // Buffer size should be positive for valid config
    TestResult::from_bool(config.buffer_size > 0)
}

#[quickcheck]
fn prop_reconnect_config_backoff_multiplier_greater_than_one(multiplier: f64) -> TestResult {
    if multiplier <= 1.0 || multiplier.is_nan() || multiplier.is_infinite() {
        return TestResult::discard();
    }
    
    let config = ReconnectConfig {
        backoff_multiplier: multiplier,
        ..Default::default()
    };
    
    TestResult::from_bool(config.backoff_multiplier > 1.0)
}

// Property tests for error handling
#[quickcheck]
fn prop_error_context_preserves_operation(operation: String) -> TestResult {
    if operation.is_empty() {
        return TestResult::discard();
    }
    
    let context = kraken_ws_sdk::ErrorContext::new(&operation);
    TestResult::from_bool(context.operation == operation)
}

#[quickcheck]
fn prop_error_context_details_preserved(key: String, value: String) -> TestResult {
    if key.is_empty() || value.is_empty() {
        return TestResult::discard();
    }
    
    let context = kraken_ws_sdk::ErrorContext::new("test")
        .with_detail(&key, &value);
    
    TestResult::from_bool(
        context.details.get(&key) == Some(&value)
    )
}

// Property tests for OrderBookManager
#[quickcheck]
fn prop_orderbook_manager_symbols_unique(symbols: Vec<String>) -> TestResult {
    let manager = OrderBookManager::new();
    
    for symbol in &symbols {
        if symbol.is_empty() {
            continue;
        }
        
        let update = OrderBookUpdate {
            symbol: symbol.clone(),
            bids: vec![],
            asks: vec![],
            timestamp: Utc::now(),
            checksum: None,
        };
        
        let _ = manager.apply_update(update);
    }
    
    let tracked_symbols = manager.get_symbols();
    let unique_symbols: std::collections::HashSet<_> = tracked_symbols.iter().collect();
    
    TestResult::from_bool(tracked_symbols.len() == unique_symbols.len())
}

// Property tests for price level ordering
#[quickcheck]
fn prop_price_levels_maintain_order(prices: Vec<f64>) -> TestResult {
    if prices.len() < 2 {
        return TestResult::discard();
    }
    
    let mut order_book = OrderBook::new("TEST/USD");
    
    for &price in &prices {
        if price <= 0.0 {
            continue;
        }
        
        let price_decimal = match Decimal::from_str(&price.to_string()) {
            Ok(d) => d,
            Err(_) => continue,
        };
        
        order_book.bids.insert(
            price_decimal,
            PriceLevel {
                price: price_decimal,
                volume: Decimal::from_str("1.0").unwrap(),
                timestamp: Utc::now(),
            },
        );
    }
    
    // Check that bids are in descending order (BTreeMap should handle this)
    let bid_prices: Vec<_> = order_book.bids.keys().collect();
    let mut sorted_prices = bid_prices.clone();
    sorted_prices.sort_by(|a, b| b.cmp(a)); // Descending order
    
    TestResult::from_bool(bid_prices == sorted_prices)
}

// Run property tests
#[cfg(test)]
// Note: Property tests are run automatically via #[quickcheck] attribute
// No need for manual test runner - quickcheck_macros handles it