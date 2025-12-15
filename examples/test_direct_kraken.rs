//! Direct test of Kraken WebSocket connection

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Testing direct Kraken WebSocket connection...");
    
    // Connect to Kraken WebSocket
    let url = "wss://ws.kraken.com";
    let (ws_stream, _) = connect_async(url).await?;
    println!("✅ Connected to Kraken WebSocket");
    
    let (mut write, mut read) = ws_stream.split();
    
    // Send subscription message for BTC/USD ticker
    let subscription = json!({
        "event": "subscribe",
        "pair": ["BTC/USD"],
        "subscription": {
            "name": "ticker"
        }
    });
    
    let msg = Message::Text(subscription.to_string());
    write.send(msg).await?;
    println!("📤 Sent subscription message: {}", subscription);
    
    // Listen for messages
    let mut message_count = 0;
    while let Some(message) = read.next().await {
        match message? {
            Message::Text(text) => {
                println!("📥 Received: {}", text);
                message_count += 1;
                
                // Stop after 5 messages to avoid infinite loop
                if message_count >= 5 {
                    break;
                }
            }
            Message::Close(_) => {
                println!("🔌 Connection closed");
                break;
            }
            _ => {}
        }
    }
    
    Ok(())
}