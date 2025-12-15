# Kraken WebSocket SDK Design Document

## Overview

The Kraken WebSocket SDK is a lightweight, high-performance library designed to provide seamless access to Kraken's real-time market data streams. The SDK abstracts the complexity of WebSocket connection management, message parsing, and data distribution while maintaining excellent performance characteristics suitable for high-frequency trading applications.

The design emphasizes modularity, testability, and ease of use, with a clean separation between transport layer, data processing, and client-facing APIs. The SDK will be implemented in Rust to leverage its memory safety, performance characteristics, and excellent async/await support for handling concurrent WebSocket operations.

## Architecture

The SDK follows a layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────┐
│           Client Application            │
├─────────────────────────────────────────┤
│              Public API                 │
├─────────────────────────────────────────┤
│          Event Dispatcher               │
├─────────────────────────────────────────┤
│           Data Parser                   │
├─────────────────────────────────────────┤
│         Message Handler                 │
├─────────────────────────────────────────┤
│        Connection Manager               │
├─────────────────────────────────────────┤
│         WebSocket Transport             │
└─────────────────────────────────────────┘
```

### Core Design Principles

1. **Asynchronous by Default**: All operations use Rust's async/await for non-blocking I/O
2. **Type Safety**: Leverage Rust's type system to prevent runtime errors
3. **Resource Efficiency**: Minimize memory allocations and CPU overhead
4. **Fault Tolerance**: Graceful handling of network issues and malformed data
5. **Extensibility**: Plugin architecture for custom data processors and handlers

## Components and Interfaces

### Connection Manager

The Connection Manager handles WebSocket lifecycle and connection state:

```rust
pub struct ConnectionManager {
    config: ConnectionConfig,
    state: ConnectionState,
    reconnect_strategy: ReconnectStrategy,
}

pub trait ConnectionManager {
    async fn connect(&mut self) -> Result<(), ConnectionError>;
    async fn disconnect(&mut self) -> Result<(), ConnectionError>;
    fn is_connected(&self) -> bool;
    fn connection_state(&self) -> ConnectionState;
}
```

**Responsibilities:**
- Establish and maintain WebSocket connections
- Handle authentication with API keys
- Implement exponential backoff for reconnection
- Monitor connection health with heartbeat/ping mechanisms

### Message Handler

Processes raw WebSocket messages and routes them appropriately:

```rust
pub struct MessageHandler {
    parser: Box<dyn DataParser>,
    dispatcher: Arc<EventDispatcher>,
}

pub trait MessageHandler {
    async fn handle_message(&self, message: WebSocketMessage) -> Result<(), ProcessingError>;
    fn register_parser(&mut self, parser: Box<dyn DataParser>);
}
```

**Responsibilities:**
- Receive raw WebSocket frames
- Validate message format and integrity
- Route messages to appropriate parsers
- Handle subscription confirmations and errors

### Data Parser

Converts raw JSON messages into structured data types:

```rust
pub trait DataParser {
    fn parse_ticker(&self, data: &str) -> Result<TickerData, ParseError>;
    fn parse_orderbook(&self, data: &str) -> Result<OrderBookUpdate, ParseError>;
    fn parse_trade(&self, data: &str) -> Result<TradeData, ParseError>;
    fn parse_ohlc(&self, data: &str) -> Result<OHLCData, ParseError>;
}

pub struct KrakenDataParser {
    // Internal parsing state and configuration
}
```

**Responsibilities:**
- Parse JSON messages according to Kraken's API specification
- Validate data integrity and handle malformed messages
- Convert timestamps and numeric values to appropriate types
- Maintain order book state for incremental updates

### Event Dispatcher

Manages event subscriptions and callback invocation:

```rust
pub struct EventDispatcher {
    subscribers: HashMap<DataType, Vec<Box<dyn EventCallback>>>,
}

pub trait EventCallback: Send + Sync {
    fn on_ticker(&self, data: TickerData);
    fn on_orderbook(&self, data: OrderBookUpdate);
    fn on_trade(&self, data: TradeData);
    fn on_error(&self, error: SdkError);
}
```

**Responsibilities:**
- Manage callback registrations for different data types
- Dispatch events to registered callbacks
- Handle callback errors without affecting other subscribers
- Provide thread-safe access for concurrent operations

### Public API

The main SDK interface exposed to client applications:

```rust
pub struct KrakenWsClient {
    connection_manager: ConnectionManager,
    message_handler: MessageHandler,
    event_dispatcher: Arc<EventDispatcher>,
}

impl KrakenWsClient {
    pub fn new(config: ClientConfig) -> Self;
    pub async fn connect(&mut self) -> Result<(), SdkError>;
    pub async fn subscribe(&self, channels: Vec<Channel>) -> Result<(), SdkError>;
    pub async fn unsubscribe(&self, channels: Vec<Channel>) -> Result<(), SdkError>;
    pub fn register_callback(&self, callback: Box<dyn EventCallback>);
    pub async fn disconnect(&mut self) -> Result<(), SdkError>;
}
```

## Data Models

### Core Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerData {
    pub symbol: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last_price: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookUpdate {
    pub symbol: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub timestamp: DateTime<Utc>,
    pub checksum: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub symbol: String,
    pub price: Decimal,
    pub volume: Decimal,
    pub side: TradeSide,
    pub timestamp: DateTime<Utc>,
    pub trade_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
}
```

### Configuration Models

```rust
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub endpoint: String,
    pub reconnect_config: ReconnectConfig,
    pub buffer_size: usize,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

After analyzing the acceptance criteria, the following properties have been identified for property-based testing. Some properties have been consolidated to eliminate redundancy and provide comprehensive validation:

**Property 1: Reconnection follows exponential backoff**
*For any* connection loss scenario, reconnection attempts should follow exponential backoff timing with increasing delays between attempts
**Validates: Requirements 1.2**

**Property 2: Authentication message format compliance**
*For any* valid API key and secret pair, authentication messages should conform to Kraken's protocol specification
**Validates: Requirements 1.3**

**Property 3: Connection error reporting completeness**
*For any* connection failure scenario, error information should include failure type, timestamp, and actionable details
**Validates: Requirements 1.4**

**Property 4: Connection state notification consistency**
*For any* connection state change, all registered listeners should be notified with the correct state information
**Validates: Requirements 1.5**

**Property 5: Subscription message protocol compliance**
*For any* valid channel specification, subscription messages should match Kraken's expected format and include all required fields
**Validates: Requirements 2.1**

**Property 6: Subscription confirmation notification**
*For any* subscription confirmation received from the exchange, the caller should be notified with the confirmed channel details
**Validates: Requirements 2.2**

**Property 7: Concurrent subscription handling**
*For any* set of simultaneous subscription requests, all requests should be processed without blocking each other
**Validates: Requirements 2.3**

**Property 8: Invalid channel rejection with error details**
*For any* invalid channel specification, the subscription should be rejected with clear error messaging describing the validation failure
**Validates: Requirements 2.4**

**Property 9: Unsubscription protocol compliance**
*For any* active subscription, unsubscribing should send the correct unsubscribe message and receive confirmation
**Validates: Requirements 2.5**

**Property 10: JSON parsing round-trip consistency**
*For any* valid Kraken WebSocket message format, parsing then serializing should preserve the original data structure
**Validates: Requirements 3.1**

**Property 11: Malformed data graceful handling**
*For any* malformed JSON input, the parser should handle the error gracefully without crashing and continue processing subsequent messages
**Validates: Requirements 3.2**

**Property 12: Ticker data structure consistency**
*For any* ticker message, the parsed output should contain all required fields (symbol, bid, ask, last_price, volume, timestamp) with correct types
**Validates: Requirements 3.3**

**Property 13: Order book state consistency**
*For any* sequence of order book updates, applying them in order should result in a consistent final state that matches the expected order book
**Validates: Requirements 3.4**

**Property 14: Trade data structure completeness**
*For any* trade message, the parsed output should contain all required fields (symbol, price, volume, side, timestamp, trade_id) with correct types
**Validates: Requirements 3.5**

**Property 15: Callback invocation for matching data types**
*For any* registered callback and matching data type, the callback should be invoked when data of that type arrives
**Validates: Requirements 4.1**

**Property 16: Multiple callback invocation ordering**
*For any* set of callbacks registered for the same data type, they should be invoked in registration order
**Validates: Requirements 4.2**

**Property 17: Exception isolation in callback processing**
*For any* callback that throws an exception, other registered callbacks should continue to execute normally
**Validates: Requirements 4.3**

**Property 18: Callback unregistration effectiveness**
*For any* unregistered callback, it should not be invoked for subsequent data of its registered type
**Validates: Requirements 4.4**

**Property 19: Error callback information completeness**
*For any* data processing failure, error callbacks should receive detailed failure information including error type, context, and timestamp
**Validates: Requirements 4.5**

**Property 20: Error logging with appropriate severity**
*For any* error occurring within the SDK, it should be logged with the correct severity level based on error type and impact
**Validates: Requirements 5.1**

**Property 21: Network error classification accuracy**
*For any* network error, it should be correctly classified as either temporary (retryable) or permanent (non-retryable)
**Validates: Requirements 5.2**

**Property 22: Rate limit handling and notification**
*For any* rate limit response from the API, the SDK should handle it gracefully and notify the caller with appropriate backoff information
**Validates: Requirements 5.3**

**Property 23: Malformed data processing continuity**
*For any* stream containing malformed data mixed with valid data, processing should continue for valid messages after logging malformed data errors
**Validates: Requirements 5.4**

**Property 24: Critical error context completeness**
*For any* critical error, the error context should include sufficient information for debugging (stack trace, state information, operation context)
**Validates: Requirements 5.5**

**Property 25: Asynchronous interface consistency**
*For any* asynchronous operation, both callback and future-based interfaces should provide equivalent functionality and results
**Validates: Requirements 6.3**

**Property 26: Resource cleanup completeness**
*For any* SDK instance, calling cleanup/shutdown methods should properly release all allocated resources (connections, memory, threads)
**Validates: Requirements 6.4**

**Property 27: Configuration override behavior**
*For any* configuration option, custom values should override defaults while unspecified options retain default values
**Validates: Requirements 6.5**

**Property 28: Memory management correctness**
*For any* sequence of SDK operations, memory usage should return to baseline levels after operations complete and cleanup is performed
**Validates: Requirements 8.5**

## Error Handling

The SDK implements a comprehensive error handling strategy with multiple layers of protection:

### Error Categories

1. **Connection Errors**: Network failures, authentication issues, protocol violations
2. **Data Errors**: Malformed JSON, missing fields, type conversion failures
3. **Configuration Errors**: Invalid parameters, missing required settings
4. **Resource Errors**: Memory allocation failures, thread pool exhaustion
5. **API Errors**: Rate limiting, invalid subscriptions, server errors

### Error Recovery Strategies

- **Automatic Retry**: Transient network errors with exponential backoff
- **Graceful Degradation**: Continue processing valid data when encountering malformed messages
- **Circuit Breaker**: Temporarily halt operations when error rates exceed thresholds
- **Fallback Mechanisms**: Use cached data or default values when real-time data is unavailable

### Error Reporting

All errors are reported through multiple channels:
- Structured logging with appropriate severity levels
- Error callbacks for application-level handling
- Metrics and monitoring integration for operational visibility
- Detailed error context for debugging and troubleshooting

## Testing Strategy

The SDK employs a dual testing approach combining unit tests and property-based tests to ensure comprehensive correctness validation:

### Unit Testing Approach

Unit tests will verify specific examples, edge cases, and integration points:
- Connection establishment and teardown scenarios
- Message parsing for known Kraken message formats
- Error handling for specific failure conditions
- API interface contracts and method signatures
- Resource cleanup and lifecycle management

Unit tests provide concrete examples of expected behavior and catch specific bugs in implementation logic.

### Property-Based Testing Approach

Property-based tests will verify universal properties across all valid inputs using the **quickcheck** crate for Rust:
- Each property-based test will run a minimum of 100 iterations to ensure statistical confidence
- Tests will use smart generators that create realistic input data within valid domains
- Each property-based test will include a comment explicitly referencing the correctness property from this design document
- Property tests will use this exact format: **Feature: kraken-ws-sdk, Property {number}: {property_text}**

**Property-Based Testing Library**: quickcheck (Rust)
**Minimum Iterations**: 100 per property test
**Generator Strategy**: Smart generators that constrain inputs to realistic WebSocket message formats and API responses

### Test Coverage Requirements

- All correctness properties must be implemented as property-based tests
- Each correctness property must be implemented by a single property-based test
- Unit tests and property tests are complementary - both must be included
- Property tests verify general correctness across input spaces
- Unit tests verify specific examples and integration scenarios

### Testing Infrastructure

- Mock WebSocket server for testing connection scenarios
- Message generators for creating valid and invalid Kraken API responses
- Test fixtures for common market data scenarios
- Performance benchmarks for high-frequency data processing
- Integration tests with actual Kraken sandbox environment

The combination of unit and property-based testing ensures both concrete correctness validation and general behavioral verification across the entire input domain.