//! # Kraken WebSocket SDK
//!
//! A lightweight, high-performance SDK for connecting to Kraken's WebSocket API
//! and processing real-time market data streams.
//!
//! ## Quick Start
//! ```rust
//! use kraken_ws_sdk::prelude::*;
//!
//! let sdk = KrakenSDK::default();
//! sdk.subscribe_ticker("BTC/USD", |t| println!("${}", t.last_price))
//!    .subscribe_orderbook("ETH/USD", 10, |b| println!("{} bids", b.bids.len()))
//!    .on_reconnect(|n| println!("Reconnect #{}", n));
//! ```

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

/// Prelude - import everything you need
pub mod prelude {
    // Core SDK
    pub use crate::sdk::{KrakenSDK, KrakenSDKBuilder};
    pub use crate::data::{TickerData, TradeData, OrderBookUpdate, OHLCData, TradeSide};
    pub use crate::error::SdkError;
    
    // Sequencing & Backpressure
    pub use crate::sequencing::{SequenceManager, SequenceState, GapEvent, ResyncEvent};
    pub use crate::backpressure::{BackpressureManager, BackpressureConfig, DropPolicy, BackpressureStats};
    
    // Latency & Metrics
    pub use crate::latency::{LatencyTracker, LatencyStats, LatencyPercentiles, format_latency};
    pub use crate::telemetry::{MetricsRegistry, SdkMetrics, TelemetryConfig};
    
    // Exchange abstraction
    pub use crate::exchange::{Exchange, ExchangeAdapter, ExchangeManager, ExchangeStatus};
    
    // Retry & Resilience
    pub use crate::retry::{RetryPolicy, CircuitBreaker, CircuitState};
    
    // Middleware
    pub use crate::middleware::{Middleware, MiddlewareChain, RequestContext};
}

use tracing_subscriber;

/// Initialize logging for the SDK
pub fn init_logging() {
    tracing_subscriber::fmt::init();
}