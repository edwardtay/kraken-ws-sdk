//! Trading API Demo
//!
//! Demonstrates the authenticated trading features of the SDK.
//!
//! ## Setup
//!
//! Set environment variables:
//! ```bash
//! export KRAKEN_API_KEY="your-api-key"
//! export KRAKEN_API_SECRET="your-api-secret"
//! ```
//!
//! ## Run
//!
//! ```bash
//! cargo run --example trading_demo
//! ```

use kraken_ws_sdk::trading_api::*;
use rust_decimal_macros::dec;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,kraken_ws_sdk=debug")
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Kraken Trading API Demo                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Check for credentials
    let api_key = std::env::var("KRAKEN_API_KEY").ok();
    let api_secret = std::env::var("KRAKEN_API_SECRET").ok();

    if api_key.is_none() || api_secret.is_none() {
        println!("⚠️  No API credentials found. Running in demo mode.");
        println!("   Set KRAKEN_API_KEY and KRAKEN_API_SECRET to test with real API.");
        println!();
        demo_mode().await;
        return Ok(());
    }

    // Create REST client
    println!("🔐 Creating authenticated REST client...");
    let client = KrakenRestClient::from_env()?;
    println!("   Rate limiter: {}", client.rate_limiter().stats());
    println!();

    // Get account balance
    println!("💰 Fetching account balances...");
    match client.get_balance().await {
        Ok(balances) => {
            println!("   Balances:");
            for (asset, balance) in &balances.assets {
                if balance.balance > dec!(0) {
                    println!("   - {}: {} (available: {})", asset, balance.balance, balance.available);
                }
            }
        }
        Err(e) => {
            println!("   Error: {}", e);
        }
    }
    println!();

    // Get open orders
    println!("📋 Fetching open orders...");
    match client.get_open_orders().await {
        Ok(orders) => {
            if orders.is_empty() {
                println!("   No open orders");
            } else {
                for order in &orders {
                    println!("   - {} {:?} {} {} @ {:?}", 
                        order.txid, order.side, order.volume, order.pair, order.price);
                }
            }
        }
        Err(e) => {
            println!("   Error: {}", e);
        }
    }
    println!();

    // Get open positions
    println!("📊 Fetching open positions...");
    match client.get_open_positions().await {
        Ok(positions) => {
            if positions.is_empty() {
                println!("   No open positions");
            } else {
                for pos in &positions {
                    println!("   - {} {:?} {} @ {} (P&L: {}%)", 
                        pos.pair, pos.side, pos.volume, pos.entry_price, pos.pnl_percent());
                }
            }
        }
        Err(e) => {
            println!("   Error: {}", e);
        }
    }
    println!();

    // Demo order creation (validate only - won't actually place)
    println!("🧪 Testing order creation (validate only)...");
    let test_order = OrderRequest::limit_buy("XBT/USD", dec!(0.0001), dec!(10000.00))
        .post_only()
        .with_client_id("sdk-demo-order")
        .validate_only();
    
    println!("   Order: {:?} {:?} {} XBT/USD @ ${}",
        test_order.side, test_order.order_type, test_order.volume, test_order.price.unwrap());
    
    match client.add_order(test_order).await {
        Ok(response) => {
            println!("   ✅ Order validated: {}", response.descr.order);
        }
        Err(e) => {
            println!("   ❌ Validation error: {}", e);
        }
    }
    println!();

    // Get WebSocket token and connect to private feeds
    println!("🔌 Getting WebSocket token...");
    match client.get_websocket_token().await {
        Ok(token) => {
            println!("   Token: {}...", &token[..20.min(token.len())]);
            
            println!();
            println!("📡 Connecting to private WebSocket...");
            
            let config = PrivateWsConfig::new(token)
                .with_channels(vec![PrivateChannel::OwnTrades, PrivateChannel::OpenOrders]);
            
            let mut ws_client = PrivateWsClient::new(config);
            
            match ws_client.connect().await {
                Ok(()) => {
                    println!("   ✅ Connected to private WebSocket");
                    
                    // Subscribe to events
                    let mut events = ws_client.subscribe();
                    
                    println!();
                    println!("👂 Listening for events (10 seconds)...");
                    println!("   (Place an order in another terminal to see execution events)");
                    
                    let timeout = tokio::time::sleep(Duration::from_secs(10));
                    tokio::pin!(timeout);
                    
                    loop {
                        tokio::select! {
                            _ = &mut timeout => {
                                println!("   Timeout reached");
                                break;
                            }
                            event = events.recv() => {
                                match event {
                                    Ok(PrivateEvent::Execution(exec)) => {
                                        println!("   🎯 EXECUTION: {} {} {} @ {} (fee: {})",
                                            exec.pair, exec.side, exec.volume, exec.price, exec.fee);
                                    }
                                    Ok(PrivateEvent::OrderUpdate(update)) => {
                                        println!("   📝 ORDER UPDATE: {} -> {:?} (filled: {})",
                                            update.txid, update.status, update.volume_exec);
                                    }
                                    Ok(PrivateEvent::Connected) => {
                                        println!("   🟢 Connected");
                                    }
                                    Ok(PrivateEvent::Disconnected) => {
                                        println!("   🔴 Disconnected");
                                        break;
                                    }
                                    Ok(PrivateEvent::Error(e)) => {
                                        println!("   ⚠️  Error: {}", e);
                                    }
                                    Ok(PrivateEvent::BalanceUpdate(update)) => {
                                        println!("   💰 BALANCE: {} = {} (available: {})",
                                            update.asset, update.balance, update.available);
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                    
                    ws_client.disconnect().await;
                }
                Err(e) => {
                    println!("   ❌ Connection error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("   Error: {}", e);
        }
    }

    println!();
    println!("✅ Demo complete!");
    
    Ok(())
}

/// Demo mode when no credentials are available
async fn demo_mode() {
    println!("📚 SDK Trading API Overview:");
    println!();
    
    println!("1️⃣  Authentication:");
    println!("   let creds = Credentials::new(api_key, api_secret)?;");
    println!("   let creds = Credentials::from_env()?;");
    println!();
    
    println!("2️⃣  REST Client:");
    println!("   let client = KrakenRestClient::new(creds);");
    println!("   let balances = client.get_balance().await?;");
    println!("   let orders = client.get_open_orders().await?;");
    println!("   let positions = client.get_open_positions().await?;");
    println!();
    
    println!("3️⃣  Order Types:");
    println!("   OrderRequest::market_buy(\"XBT/USD\", dec!(0.001))");
    println!("   OrderRequest::limit_sell(\"ETH/USD\", dec!(1.0), dec!(2500.00))");
    println!("   OrderRequest::limit_buy(...).post_only().with_client_id(\"my-order\")");
    println!();
    
    println!("4️⃣  Order Management:");
    println!("   client.add_order(order).await?;");
    println!("   client.cancel_order(\"TXID\").await?;");
    println!("   client.cancel_all().await?;");
    println!("   client.edit_order(EditOrderRequest::new(\"TXID\").with_price(new_price)).await?;");
    println!();
    
    println!("5️⃣  Private WebSocket:");
    println!("   let token = client.get_websocket_token().await?;");
    println!("   let mut ws = PrivateWsClient::new(PrivateWsConfig::new(token));");
    println!("   ws.connect().await?;");
    println!("   let mut events = ws.subscribe();");
    println!("   while let Ok(event) = events.recv().await {{");
    println!("       match event {{");
    println!("           PrivateEvent::Execution(exec) => {{ /* fill */ }}");
    println!("           PrivateEvent::OrderUpdate(update) => {{ /* status */ }}");
    println!("       }}");
    println!("   }}");
    println!();
    
    println!("6️⃣  Rate Limiting:");
    println!("   // Automatic! SDK handles Kraken's tier-based limits");
    println!("   // Configure tier: KrakenRestClient::with_tier(creds, AccountTier::Pro)");
    println!();
}
