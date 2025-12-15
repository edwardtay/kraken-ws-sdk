//! Multi-Exchange Abstraction Demo
//!
//! Demonstrates the ExchangeAdapter trait pattern for supporting
//! multiple exchanges with a unified interface.

use kraken_ws_sdk::{
    Exchange, ExchangeAdapter, ExchangeManager, ExchangeStatus,
    KrakenAdapter, BinanceAdapter, CoinbaseAdapter, create_adapter,
};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  🔄 Multi-Exchange Abstraction Demo");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 1. Create individual adapters
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("📦 Creating exchange adapters...\n");
    
    let kraken = KrakenAdapter::new();
    let binance = BinanceAdapter::new();
    let coinbase = CoinbaseAdapter::new();
    
    // Show capabilities
    println!("Exchange Capabilities:");
    println!("┌─────────────┬─────────┬────────┬───────────┬──────┬────────────┐");
    println!("│ Exchange    │ Ticker  │ Trades │ OrderBook │ OHLC │ Rate Limit │");
    println!("├─────────────┼─────────┼────────┼───────────┼──────┼────────────┤");
    
    for (name, adapter) in [("Kraken", &kraken as &dyn ExchangeAdapter), 
                            ("Binance", &binance as &dyn ExchangeAdapter),
                            ("Coinbase", &coinbase as &dyn ExchangeAdapter)] {
        let caps = adapter.capabilities();
        println!("│ {:11} │ {:^7} │ {:^6} │ {:^9} │ {:^4} │ {:>6}/s   │",
            name,
            if caps.supports_ticker { "✅" } else { "❌" },
            if caps.supports_trades { "✅" } else { "❌" },
            if caps.supports_orderbook { "✅" } else { "❌" },
            if caps.supports_ohlc { "✅" } else { "❌" },
            caps.rate_limit_per_second,
        );
    }
    println!("└─────────────┴─────────┴────────┴───────────┴──────┴────────────┘");
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 2. Use ExchangeManager for multi-exchange
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("🔧 Setting up ExchangeManager...\n");
    
    let mut manager = ExchangeManager::new();
    
    // Add exchanges using factory
    manager.add_exchange(create_adapter(Exchange::Kraken));
    manager.add_exchange(create_adapter(Exchange::Binance));
    manager.add_exchange(create_adapter(Exchange::Coinbase));
    
    println!("Registered exchanges: {:?}", manager.exchanges());
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 3. Check status before connecting
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("📊 Exchange Status (before connect):");
    for (exchange, status) in manager.status_all() {
        let status_icon = match status {
            ExchangeStatus::Connected => "🟢",
            ExchangeStatus::Connecting => "🟡",
            ExchangeStatus::Disconnected => "⚪",
            ExchangeStatus::Reconnecting => "🟠",
            ExchangeStatus::Error => "🔴",
            ExchangeStatus::NotImplemented => "⬜",
        };
        println!("  {} {:?}: {:?}", status_icon, exchange, status);
    }
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 4. Connect to all exchanges
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("🔌 Connecting to all exchanges...\n");
    
    let results = manager.connect_all().await;
    
    println!("Connection Results:");
    for (exchange, result) in &results {
        match result {
            Ok(_) => println!("  ✅ {:?}: Connected", exchange),
            Err(e) => println!("  ❌ {:?}: {}", exchange, e),
        }
    }
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 5. Subscribe to data (Kraken only - it's implemented)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("📡 Subscribing to BTC/USD on Kraken...\n");
    
    if let Some(kraken) = manager.get_mut(Exchange::Kraken) {
        // Set up callbacks
        kraken.on_ticker(Arc::new(|exchange, ticker| {
            println!("  [{:?}] Ticker: {} @ ${}", exchange, ticker.symbol, ticker.last_price);
        }));
        
        kraken.on_trade(Arc::new(|exchange, trade| {
            println!("  [{:?}] Trade: {} {:?} {} @ ${}", 
                exchange, trade.symbol, trade.side, trade.volume, trade.price);
        }));
        
        // Subscribe
        let _ = kraken.subscribe_ticker(&"BTC/USD".to_string()).await;
        let _ = kraken.subscribe_trades(&"BTC/USD".to_string()).await;
        let _ = kraken.subscribe_orderbook(&"ETH/USD".to_string(), 10).await;
        
        println!("Subscribed symbols: {:?}", kraken.subscribed_symbols());
    }
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 6. Final status
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("📊 Final Exchange Status:");
    for (exchange, status) in manager.status_all() {
        let status_icon = match status {
            ExchangeStatus::Connected => "🟢",
            ExchangeStatus::Connecting => "🟡",
            ExchangeStatus::Disconnected => "⚪",
            ExchangeStatus::Reconnecting => "🟠",
            ExchangeStatus::Error => "🔴",
            ExchangeStatus::NotImplemented => "⬜",
        };
        println!("  {} {:?}: {:?}", status_icon, exchange, status);
    }
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 7. Disconnect
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    println!("🔌 Disconnecting...");
    let _ = manager.disconnect_all().await;
    
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  ✅ Multi-Exchange Demo Complete!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("The ExchangeAdapter trait allows:");
    println!("  • Unified API across all exchanges");
    println!("  • Easy addition of new exchanges");
    println!("  • Symbol normalization (BTC/USD everywhere)");
    println!("  • Capability discovery per exchange");
    println!("  • Centralized connection management");
}
