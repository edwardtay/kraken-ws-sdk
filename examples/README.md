# Kraken WebSocket SDK Examples

This directory contains examples demonstrating how to use the Kraken WebSocket SDK.

## Running Examples

To run an example, use:

```bash
cargo run --example basic_usage
```

## Available Examples

### basic_usage.rs

Demonstrates the fundamental usage of the SDK:
- Creating a client with configuration
- Registering callbacks for different data types
- Connecting to the WebSocket API
- Subscribing to market data channels
- Handling real-time data updates
- Proper cleanup and resource management

### Key Features Demonstrated

- **Connection Management**: Establishing and maintaining WebSocket connections
- **Event Handling**: Registering callbacks for different types of market data
- **Subscription Management**: Subscribing to specific channels and symbols
- **Data Processing**: Receiving and processing real-time market data
- **Error Handling**: Proper error handling and logging
- **Resource Cleanup**: Proper cleanup of resources when done

## Configuration Options

The SDK supports various configuration options:

```rust
let config = ClientConfig {
    endpoint: "wss://ws.kraken.com".to_string(),
    api_key: Some("your_api_key".to_string()),      // Optional for private channels
    api_secret: Some("your_api_secret".to_string()), // Optional for private channels
    timeout: Duration::from_secs(30),
    buffer_size: 1024,
    reconnect_config: ReconnectConfig {
        max_attempts: 10,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
        backoff_multiplier: 2.0,
    },
};
```

## Available Channels

The SDK supports various Kraken WebSocket channels:

- **ticker**: Real-time ticker data
- **trade**: Real-time trade data
- **book**: Order book updates
- **ohlc**: OHLC (candlestick) data
- **spread**: Spread data

## Error Handling

The SDK provides comprehensive error handling:

```rust
impl EventCallback for MyCallback {
    fn on_error(&self, error: SdkError) {
        match error {
            SdkError::Connection(conn_err) => {
                eprintln!("Connection error: {}", conn_err);
            }
            SdkError::Parse(parse_err) => {
                eprintln!("Parse error: {}", parse_err);
            }
            SdkError::Subscription(sub_err) => {
                eprintln!("Subscription error: {}", sub_err);
            }
            _ => {
                eprintln!("Other error: {}", error);
            }
        }
    }
}
```

## Best Practices

1. **Always register error callbacks** to handle any issues that may occur
2. **Use proper cleanup** by calling `client.cleanup().await` when done
3. **Handle connection state changes** to respond to network issues
4. **Validate configuration** before creating the client
5. **Use appropriate buffer sizes** based on expected message volume
6. **Implement reconnection logic** for production applications