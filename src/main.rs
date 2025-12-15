use kraken_ws_sdk::{init_logging, KrakenWsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    
    println!("Kraken WebSocket SDK - Example Application");
    
    // This will be expanded with actual usage examples
    Ok(())
}