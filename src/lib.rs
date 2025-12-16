//! # Kraken WebSocket SDK
//!
//! A lightweight, high-performance SDK for connecting to Kraken's WebSocket API
//! and processing real-time market data streams.
//!
//! ## Features
//! - `wasm` - Enable WebAssembly support (mutually exclusive with native tokio runtime)
//! - `chaos` - Enable chaos/fault injection features
//! - `metrics` - Enable Prometheus-style metrics export
//!
//! ## Quick Start
//! ```rust,ignore
//! use kraken_ws_sdk::prelude::*;
//!
//! let sdk = KrakenSDK::default();
//! sdk.subscribe_ticker("BTC/USD", |t| println!("${}", t.last_price))
//!    .subscribe_orderbook("ETH/USD", 10, |b| println!("{} bids", b.bids.len()))
//!    .on_reconnect(|n| println!("Reconnect #{}", n));
//! ```

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