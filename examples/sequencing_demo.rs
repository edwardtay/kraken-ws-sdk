//! Deterministic Message Sequencing Demo
//! 
//! Production-grade sequence validation and gap detection.
//! Run with: cargo run --example sequencing_demo

use kraken_ws_sdk::prelude::*;
use kraken_ws_sdk::{SequenceManager, SequenceConfig};

fn main() {
    println!("🔢 Kraken SDK - Deterministic Message Sequencing Demo\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    // Create sequence manager with custom config
    let config = SequenceConfig {
        max_gap_size: 10,           // Resync if gap > 10
        max_pending_messages: 100,  // Resync if too many pending
        pending_timeout_secs: 30,   // Timeout for pending messages
        auto_resync: true,          // Auto-resync on large gaps
    };
    
    let seq_manager = SequenceManager::with_config(config);
    
    // Set up gap detection callback
    seq_manager.on_gap(|event| {
        println!("⚠️  GAP DETECTED!");
        println!("   Channel: {}", event.channel);
        println!("   Expected: {}, Received: {}", event.expected_sequence, event.received_sequence);
        println!("   Gap Size: {}", event.gap_size);
        println!();
    });
    
    // Set up resync callback
    seq_manager.on_resync(|event| {
        println!("🔄 RESYNC TRIGGERED!");
        println!("   Channel: {}", event.channel);
        println!("   Last Good Sequence: {}", event.last_good_sequence);
        println!("   Reason: {:?}", event.reason);
        println!();
    });
    
    // ═══════════════════════════════════════════════════════════════
    // Simulate message processing
    // ═══════════════════════════════════════════════════════════════
    
    println!("📨 Processing messages for BTC/USD channel...\n");
    
    // Normal in-order messages
    for seq in 1..=5 {
        let result = seq_manager.validate("BTC/USD", seq, &format!("msg_{}", seq));
        print_result(&result, seq);
    }
    
    println!("\n📨 Simulating network issue - message 6 lost, receiving 7...\n");
    
    // Gap: message 6 is lost, we receive 7
    let result = seq_manager.validate("BTC/USD", 7, "msg_7");
    print_result(&result, 7);
    
    println!("\n📨 Late arrival - message 6 finally arrives...\n");
    
    // Late message 6 arrives
    let result = seq_manager.validate("BTC/USD", 6, "msg_6");
    print_result(&result, 6);
    
    // Check state after gap recovery
    let state = seq_manager.get_state("BTC/USD").unwrap();
    println!("\n📊 State after gap recovery:");
    println!("   last_sequence: {}", state.last_sequence);
    println!("   gap_detected: {}", state.gap_detected);
    println!("   resync_triggered: {}", state.resync_triggered);
    println!("   messages_processed: {}", state.messages_processed);
    println!("   total_gaps: {}", state.total_gaps);
    
    // ═══════════════════════════════════════════════════════════════
    // Simulate large gap triggering resync
    // ═══════════════════════════════════════════════════════════════
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📨 Simulating large gap on ETH/USD channel...\n");
    
    seq_manager.validate("ETH/USD", 1, "eth_1");
    seq_manager.validate("ETH/USD", 2, "eth_2");
    
    // Large gap - should trigger resync
    let result = seq_manager.validate("ETH/USD", 100, "eth_100");
    print_result(&result, 100);
    
    // ═══════════════════════════════════════════════════════════════
    // Statistics
    // ═══════════════════════════════════════════════════════════════
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📈 Overall Statistics:\n");
    
    let stats = seq_manager.stats();
    println!("   Total Channels: {}", stats.total_channels);
    println!("   Total Messages: {}", stats.total_messages);
    println!("   Total Gaps: {}", stats.total_gaps);
    println!("   Channels with Gaps: {}", stats.channels_with_gaps);
    println!("   Gap Rate: {:.4}%", stats.gap_rate * 100.0);
    
    println!("\n✅ Demo complete!");
}

fn print_result(result: &kraken_ws_sdk::SequenceResult, seq: u64) {
    let status = if result.in_order { "✅" } else { "❌" };
    println!(
        "   {} Seq #{}: in_order={}, expected={}, gap_size={}, resync={}",
        status,
        seq,
        result.in_order,
        result.expected,
        result.gap_size,
        result.resync_triggered
    );
}