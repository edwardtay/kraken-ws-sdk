//! Kraken WebSocket SDK - Clean API Demo
//! 
//! This example demonstrates the minimal but powerful SDK API.
//! Run with: cargo run --example sdk_demo

use kraken_ws_sdk::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🦑 Kraken WebSocket SDK - API Demo\n");
    
    // Track message count
    let msg_count = Arc::new(AtomicU64::new(0));
    let count_clone = msg_count.clone();
    
    // ═══════════════════════════════════════════════════════════════
    // SDK USAGE - Clean, Minimal API
    // ═══════════════════════════════════════════════════════════════
    
    let sdk = KrakenSDK::default();
    
    // Subscribe to ticker with closure callback
    sdk.subscribe_ticker("BTC/USD", move |ticker| {
        println!("📈 BTC/USD: ${:.2} (bid: ${:.2}, ask: ${:.2})", 
            ticker.last_price, ticker.bid, ticker.ask);
    });
    
    // Subscribe to order book with depth
    sdk.subscribe_orderbook("ETH/USD", 10, |book| {
        let best_bid = book.bids.first().map(|b| b.price.to_string()).unwrap_or_default();
        let best_ask = book.asks.first().map(|a| a.price.to_string()).unwrap_or_default();
        println!("📊 ETH/USD Order Book: best bid={}, best ask={}", best_bid, best_ask);
    });
    
    // Subscribe to trades
    sdk.subscribe_trades("BTC/USD", move |trade| {
        count_clone.fetch_add(1, Ordering::Relaxed);
        println!("💰 Trade: {:?} {} BTC @ ${}", trade.side, trade.volume, trade.price);
    });
    
    // Handle reconnection events
    sdk.on_reconnect(|attempt| {
        println!("🔄 Reconnecting... attempt #{}", attempt);
    });
    
    // Handle errors
    sdk.on_error(|err| {
        eprintln!("❌ Error: {}", err);
    });
    
    // ═══════════════════════════════════════════════════════════════
    // Connect and run
    // ═══════════════════════════════════════════════════════════════
    
    println!("Subscribed pairs: {:?}", sdk.subscribed_pairs());
    println!("Connecting to Kraken...\n");
    
    sdk.connect().await?;
    
    println!("✅ Connected! Receiving live data...\n");
    
    // Run for 30 seconds
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    
    // Unsubscribe from a pair
    sdk.unsubscribe("ETH/USD");
    println!("\n📤 Unsubscribed from ETH/USD");
    println!("Active pairs: {:?}", sdk.subscribed_pairs());
    
    // Disconnect
    sdk.disconnect().await?;
    
    println!("\n📊 Total trades received: {}", msg_count.load(Ordering::Relaxed));
    println!("👋 Disconnected. Goodbye!");
    
    Ok(())
}