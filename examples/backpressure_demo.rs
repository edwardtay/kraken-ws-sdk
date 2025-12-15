//! Backpressure and Throttling Control Demo
//! 
//! Production-grade flow control for high-frequency trading.
//! Run with: cargo run --example backpressure_demo

use kraken_ws_sdk::{
    BackpressureManager, BackpressureConfig, DropPolicy, BufferedMessage,
};
use std::time::Instant;
use chrono::Utc;

fn main() {
    println!("⚡ Kraken SDK - Backpressure & Throttling Demo\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    // ═══════════════════════════════════════════════════════════════
    // Configure backpressure
    // ═══════════════════════════════════════════════════════════════
    
    let config = BackpressureConfig {
        max_messages_per_second: 100,  // Rate limit
        max_buffer_size: 50,           // Buffer limit
        drop_policy: DropPolicy::Oldest,
        coalesce_updates: true,        // Merge updates for same symbol
        burst_allowance: 20,           // Allow short bursts
        rate_window_ms: 1000,
    };
    
    println!("📋 Configuration:");
    println!("   max_messages_per_second: {}", config.max_messages_per_second);
    println!("   max_buffer_size: {}", config.max_buffer_size);
    println!("   drop_policy: {:?}", config.drop_policy);
    println!("   coalesce_updates: {}", config.coalesce_updates);
    println!("   burst_allowance: {}", config.burst_allowance);
    println!();
    
    let bp = BackpressureManager::with_config(config);
    
    // ═══════════════════════════════════════════════════════════════
    // Set up callbacks
    // ═══════════════════════════════════════════════════════════════
    
    bp.on_drop(|event| {
        println!("🗑️  DROPPED: {} {} - {:?}", 
            event.channel, event.symbol, event.reason);
    });
    
    bp.on_coalesce(|event| {
        println!("🔀 COALESCED: {} {} (seq {} -> {})", 
            event.channel, event.symbol,
            event.old_sequence.unwrap_or(0),
            event.new_sequence.unwrap_or(0));
    });
    
    bp.on_rate_limit(|event| {
        println!("⚠️  RATE LIMIT: {} - {:.1} msg/s (limit: {})", 
            event.channel, event.current_rate, event.limit);
    });
    
    // ═══════════════════════════════════════════════════════════════
    // Simulate high-frequency message stream
    // ═══════════════════════════════════════════════════════════════
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📨 Simulating high-frequency ticker updates...\n");
    
    let symbols = ["BTC/USD", "ETH/USD", "ADA/USD"];
    let mut seq = 0u64;
    
    // Send 200 messages rapidly (will trigger rate limiting)
    for i in 0..200 {
        seq += 1;
        let symbol = symbols[i % symbols.len()];
        
        let msg = BufferedMessage {
            channel: "ticker".to_string(),
            symbol: symbol.to_string(),
            data: format!("price_update_{}", i),
            sequence: Some(seq),
            received_at: Instant::now(),
            timestamp: Utc::now(),
        };
        
        let result = bp.process(msg);
        
        // Print every 50th message
        if i % 50 == 0 {
            println!("   Message #{}: accepted={}, dropped={}, coalesced={}, rate={:.1}/s",
                i, result.accepted, result.dropped, result.coalesced, result.current_rate);
        }
    }
    
    // ═══════════════════════════════════════════════════════════════
    // Show statistics
    // ═══════════════════════════════════════════════════════════════
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📊 Backpressure Statistics:\n");
    
    let stats = bp.global_stats();
    println!("   Total Received:    {}", stats.total_received);
    println!("   Total Accepted:    {}", stats.total_accepted);
    println!("   Total Dropped:     {}", stats.total_dropped);
    println!("   Total Coalesced:   {}", stats.total_coalesced);
    println!("   Peak Rate:         {:.1} msg/s", stats.peak_rate);
    println!("   Drop Rate:         {:.2}%", stats.drop_rate);
    println!("   Coalesce Rate:     {:.2}%", stats.coalesce_rate);
    println!("   Peak Queue Depth:  {}", stats.peak_queue_depth);
    
    // ═══════════════════════════════════════════════════════════════
    // Demonstrate different drop policies
    // ═══════════════════════════════════════════════════════════════
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🔧 Drop Policy Comparison:\n");
    
    for policy in [DropPolicy::Oldest, DropPolicy::Latest, DropPolicy::Random] {
        let config = BackpressureConfig {
            max_messages_per_second: 50,
            max_buffer_size: 10,
            drop_policy: policy,
            coalesce_updates: false,
            ..Default::default()
        };
        
        let bp = BackpressureManager::with_config(config);
        
        // Send 100 messages
        for i in 0..100 {
            let msg = BufferedMessage {
                channel: "test".to_string(),
                symbol: format!("SYM{}", i),
                data: "test".to_string(),
                sequence: Some(i as u64),
                received_at: Instant::now(),
                timestamp: Utc::now(),
            };
            bp.process(msg);
        }
        
        let stats = bp.global_stats();
        println!("   {:?}: accepted={}, dropped={}", 
            policy, stats.total_accepted, stats.total_dropped);
    }
    
    println!("\n✅ Demo complete!");
    println!("\n💡 Key Takeaways:");
    println!("   • Coalescing reduces redundant updates for same symbol");
    println!("   • Rate limiting prevents overwhelming downstream systems");
    println!("   • Drop policies let you choose what to sacrifice under load");
    println!("   • Statistics help monitor system health in production");
}