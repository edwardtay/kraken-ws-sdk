//! # Kraken WebSocket SDK
//!
//! A lightweight, high-performance SDK for connecting to Kraken's WebSocket API
//! and processing real-time market data streams.
//!
//! ## Features
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `public` (default) | Public market data (ticker, trades, book) |
//! | `private` | Authenticated channels (ownTrades, openOrders) |
//! | `orderbook-state` | Full orderbook state management |
//! | `metrics` | Prometheus-style metrics export |
//! | `wasm` | WebAssembly support (browser) |
//! | `chaos` | Fault injection for testing |
//!
//! ## Quick Start
//!
//! ```rust
//! use kraken_ws_sdk::prelude::*;
//!
//! // Create a channel subscription
//! let channel = Channel::new("ticker").with_symbol("BTC/USD");
//! assert_eq!(channel.name, "ticker");
//! assert_eq!(channel.symbol, Some("BTC/USD".to_string()));
//! ```
//!
//! ## Configuration
//!
//! ```rust
//! use kraken_ws_sdk::prelude::*;
//! use std::time::Duration;
//!
//! // Default configuration
//! let config = ClientConfig::default();
//! assert_eq!(config.endpoint, "wss://ws.kraken.com");
//!
//! // Custom configuration
//! let config = ClientConfig {
//!     endpoint: "wss://ws.kraken.com".to_string(),
//!     timeout: Duration::from_secs(30),
//!     buffer_size: 1024,
//!     ..Default::default()
//! };
//! ```
//!
//! ## Backpressure Control
//!
//! ```rust
//! use kraken_ws_sdk::backpressure::{BackpressureConfig, DropPolicy};
//!
//! let config = BackpressureConfig {
//!     max_messages_per_second: 1000,
//!     max_buffer_size: 500,
//!     drop_policy: DropPolicy::Oldest,
//!     ..Default::default()
//! };
//!
//! assert_eq!(config.max_messages_per_second, 1000);
//! ```
//!
//! ## Latency Tracking
//!
//! ```rust
//! use kraken_ws_sdk::latency::format_latency;
//!
//! // Format latency values for display
//! let fast = format_latency(50.0);
//! assert!(fast.contains("ms") || fast.contains("µs"));
//!
//! let slow = format_latency(1500.0);
//! assert!(slow.contains("s"));
//! ```
//!
//! ## Security
//!
//! **Important:** Never log or expose API credentials.
//!
//! ```rust
//! // ✅ Load credentials from environment
//! let api_key = std::env::var("KRAKEN_API_KEY").ok();
//! let api_secret = std::env::var("KRAKEN_API_SECRET").ok();
//!
//! // The SDK automatically redacts credentials in logs
//! ```
//!
//! ## MSRV
//!
//! Minimum Supported Rust Version: **1.70**

// Compile-time check: wasm feature uses different runtime, not compatible with native tokio
#[cfg(all(feature = "wasm", not(target_arch = "wasm32")))]
compile_error!("The `wasm` feature should only be enabled when targeting wasm32. Use `--target wasm32-unknown-unknown`.");

pub mod backpressure;
pub mod client;
pub mod connection;
pub mod data;
pub mod error;
pub mod events;
pub mod exchange;
pub mod latency;
pub mod middleware;
pub mod orderbook;
pub mod parser;
pub mod retry;
pub mod sdk;
pub mod sequencing;
pub mod subscription;
pub mod telemetry;

#[cfg(feature = "wasm")]
pub mod wasm;

// Legacy exports
pub use client::{KrakenWsClient, ClientConfigBuilder};
pub use data::*;
pub use error::*;
pub use events::*;
pub use orderbook::*;

// New clean SDK API
pub use sdk::{KrakenSDK, KrakenSDKBuilder};

// Sequencing exports
pub use sequencing::{
    SequenceManager, SequenceConfig, SequenceState, SequenceResult,
    SequenceStats, GapEvent, ResyncEvent, ResyncReason,
};

// Backpressure exports
pub use backpressure::{
    BackpressureManager, BackpressureConfig, BackpressureResult, BackpressureStats,
    DropPolicy, DropEvent, DropReason, CoalesceEvent, RateLimitEvent, BufferedMessage,
};

// Latency exports
pub use latency::{
    LatencyTracker, LatencyConfig, LatencyMeasurement, LatencyStats,
    LatencyPercentiles, LatencyHistogram, HistogramBucket,
    LatencyAlert, LatencyAlertType, format_latency,
};

// Exchange abstraction exports
pub use exchange::{
    Exchange, ExchangeAdapter, ExchangeConfig, ExchangeCapabilities, ExchangeStatus,
    ExchangeManager, KrakenAdapter, BinanceAdapter, CoinbaseAdapter,
    SymbolMapper, Symbol, NativeSymbol, create_adapter,
};

// Retry policy exports
pub use retry::{
    RetryPolicy, RetryPolicyBuilder, RetryExecutor, RetryableError,
    CircuitBreaker, CircuitState,
};

// Middleware exports
pub use middleware::{
    Middleware, MiddlewareChain, RequestContext, ResponseContext,
    LoggingMiddleware, MetricsMiddleware, RateLimitMiddleware,
    OperationMetrics, MetricsSnapshot,
};

// Telemetry exports
pub use telemetry::{
    TelemetryConfig, TelemetryConfigBuilder, MetricsRegistry,
    Counter, Gauge, Histogram, SdkMetrics, Span,
};

/// Prelude - minimal public API surface
/// 
/// Import with: `use kraken_ws_sdk::prelude::*;`
/// 
/// This provides the essential types for most use cases:
/// - `KrakenSDK` - Main entry point
/// - `ClientConfig` - Configuration builder
/// - `Channel` - Subscription channel builder
/// - `Event` - Unified event enum
/// - Core data types (TickerData, TradeData, etc.)
pub mod prelude {
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // CORE API (always available)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    
    /// Main SDK entry point
    pub use crate::sdk::{KrakenSDK, KrakenSDKBuilder};
    
    /// Configuration
    pub use crate::client::ClientConfigBuilder;
    pub use crate::data::ClientConfig;
    
    /// Channel builders
    pub use crate::data::Channel;
    
    /// Core data types
    pub use crate::data::{TickerData, TradeData, OrderBookUpdate, OHLCData, TradeSide};
    
    /// Errors
    pub use crate::error::SdkError;
    
    /// Connection state
    pub use crate::data::ConnectionState;
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // OPTIONAL: Backpressure (for high-throughput)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    pub use crate::backpressure::{BackpressureConfig, DropPolicy};
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // OPTIONAL: Latency tracking
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    pub use crate::latency::{LatencyStats, format_latency};
}

/// Extended prelude with all features
/// 
/// Import with: `use kraken_ws_sdk::prelude_full::*;`
pub mod prelude_full {
    pub use crate::prelude::*;
    
    // Sequencing
    pub use crate::sequencing::{SequenceManager, SequenceState, GapEvent, ResyncEvent};
    
    // Full backpressure
    pub use crate::backpressure::{BackpressureManager, BackpressureStats};
    
    // Full latency
    pub use crate::latency::{LatencyTracker, LatencyPercentiles, LatencyHistogram};
    
    // Telemetry
    pub use crate::telemetry::{MetricsRegistry, SdkMetrics, TelemetryConfig};
    
    // Exchange abstraction
    pub use crate::exchange::{Exchange, ExchangeAdapter, ExchangeManager, ExchangeStatus};
    
    // Retry & Resilience
    pub use crate::retry::{RetryPolicy, CircuitBreaker, CircuitState};
    
    // Middleware
    pub use crate::middleware::{Middleware, MiddlewareChain};
}

use tracing_subscriber;

/// Initialize logging for the SDK
pub fn init_logging() {
    tracing_subscriber::fmt::init();
}