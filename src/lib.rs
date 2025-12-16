//! # Kraken WebSocket SDK
//!
//! A production-grade SDK for Kraken's WebSocket API with deterministic
//! connection management and a minimal, stable API surface.
//!
//! ## API Stability
//!
//! This SDK follows a **frozen API** philosophy:
//! - Types in `prelude` are stable and won't break between minor versions
//! - Internal modules (`internal::*`) may change without notice
//! - Use `prelude` for production code
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use kraken_ws_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), SdkError> {
//!     let mut client = KrakenClient::connect(ClientConfig::default()).await?;
//!     
//!     client.subscribe(vec![
//!         Channel::ticker("BTC/USD"),
//!         Channel::trades("ETH/USD"),
//!     ]).await?;
//!     
//!     let mut events = client.events();
//!     while let Some(event) = events.recv().await {
//!         match event {
//!             Event::Ticker(t) => println!("{}: ${}", t.symbol, t.last_price),
//!             Event::Trade(t) => println!("Trade: {} @ ${}", t.symbol, t.price),
//!             Event::StateChange(s) => println!("State: {:?}", s),
//!             Event::Error(e) => eprintln!("Error: {}", e),
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Connection State Machine
//!
//! The SDK uses a deterministic state machine for connection management:
//!
//! ```text
//! DISCONNECTED ──connect()──▶ CONNECTING ──success──▶ AUTHENTICATING
//!      ▲                          │                        │
//!      │                       failure                  success
//!      │                          │                        ▼
//!      │                          ▼                   SUBSCRIBING
//!      │                      DEGRADED                     │
//!      │                          │                     success
//!      │                       retry                       ▼
//!      │                          │                   SUBSCRIBED ◀──resync──┐
//!      │                          ▼                        │                │
//!      └────────close()────── CLOSED ◀──max_retries────────┴──gap_detected──┘
//!                                                          │
//!                                                      RESYNCING
//! ```
//!
//! Each state transition emits an `Event::StateChange(ConnectionState)`.
//!
//! ## Security
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// COMPILE-TIME CHECKS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(all(feature = "wasm", not(target_arch = "wasm32")))]
compile_error!("The `wasm` feature should only be enabled when targeting wasm32.");

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PUBLIC API - STABLE (prelude)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Stable public API - import this for production code
/// 
/// ```rust
/// use kraken_ws_sdk::prelude::*;
/// ```
pub mod prelude {
    // ── Client ──────────────────────────────────────────────────────────────
    /// Main client - the only entry point you need
    pub use crate::client::KrakenWsClient as KrakenClient;
    
    // ── Configuration ───────────────────────────────────────────────────────
    /// Client configuration
    pub use crate::data::ClientConfig;
    /// Reconnection configuration
    pub use crate::data::ReconnectConfig;
    
    // ── Channels ────────────────────────────────────────────────────────────
    /// Channel subscription builder
    pub use crate::data::Channel;
    
    // ── Events ──────────────────────────────────────────────────────────────
    /// Unified event type (recommended)
    pub use crate::events::SdkEvent as Event;
    /// Event stream receiver
    pub use crate::events::EventReceiver;
    /// Callback trait (legacy)
    pub use crate::events::EventCallback;
    
    // ── Connection State ────────────────────────────────────────────────────
    /// Connection state machine states
    pub use crate::state::ConnectionState;
    /// State transition events
    pub use crate::state::StateTransition;
    
    // ── Data Types ──────────────────────────────────────────────────────────
    /// Ticker update
    pub use crate::data::TickerData;
    /// Trade execution
    pub use crate::data::TradeData;
    /// Order book update
    pub use crate::data::OrderBookUpdate;
    /// OHLC candle
    pub use crate::data::OHLCData;
    /// Trade side (Buy/Sell)
    pub use crate::data::TradeSide;
    /// Price level in order book
    pub use crate::data::PriceLevel;
    /// Data type enum
    pub use crate::data::DataType;
    
    // ── Errors ──────────────────────────────────────────────────────────────
    /// SDK error type
    pub use crate::error::SdkError;
    
    // ── Backpressure (optional) ─────────────────────────────────────────────
    /// Backpressure configuration
    pub use crate::backpressure::{BackpressureConfig, DropPolicy};
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// EXTENDED API - STABLE (for advanced use cases)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Extended API for advanced use cases
pub mod extended {
    pub use crate::prelude::*;
    
    // Sequencing & gap detection
    pub use crate::sequencing::{SequenceManager, SequenceConfig, GapEvent};
    
    // Latency tracking
    pub use crate::latency::{LatencyTracker, LatencyStats, LatencyPercentiles};
    
    // Retry policies
    pub use crate::retry::{RetryPolicy, CircuitBreaker, CircuitState};
    
    // Telemetry
    pub use crate::telemetry::{TelemetryConfig, MetricsRegistry};
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// INTERNAL MODULES - UNSTABLE (may change without notice)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Internal implementation details - DO NOT depend on these directly
#[doc(hidden)]
pub mod internal {
    pub use crate::connection;
    pub use crate::parser;
    pub use crate::subscription;
    pub use crate::orderbook;
    pub use crate::middleware;
    pub use crate::exchange;
    pub use crate::sdk;
}

// Module declarations (internal)
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
pub mod state;  // NEW: Connection state machine
pub mod subscription;
pub mod telemetry;

#[cfg(feature = "wasm")]
pub mod wasm;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// LEGACY EXPORTS - DEPRECATED (use prelude instead)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Keep for backwards compatibility, but prefer prelude
#[doc(hidden)]
pub use client::{KrakenWsClient, ClientConfigBuilder};
#[doc(hidden)]
pub use data::*;
#[doc(hidden)]
pub use error::*;
#[doc(hidden)]
pub use events::*;
#[doc(hidden)]
pub use orderbook::*;

// Legacy SDK API
#[doc(hidden)]
pub use sdk::{KrakenSDK, KrakenSDKBuilder};

// Legacy sequencing exports
#[doc(hidden)]
pub use sequencing::{
    SequenceManager, SequenceConfig, SequenceState, SequenceResult,
    SequenceStats, GapEvent, ResyncEvent, ResyncReason,
};

// Legacy backpressure exports
#[doc(hidden)]
pub use backpressure::{
    BackpressureManager, BackpressureConfig, BackpressureResult, BackpressureStats,
    DropPolicy, DropEvent, DropReason, CoalesceEvent, RateLimitEvent, BufferedMessage,
};

// Legacy latency exports
#[doc(hidden)]
pub use latency::{
    LatencyTracker, LatencyConfig, LatencyMeasurement, LatencyStats,
    LatencyPercentiles, LatencyHistogram, HistogramBucket,
    LatencyAlert, LatencyAlertType, format_latency,
};

// Legacy exchange exports
#[doc(hidden)]
pub use exchange::{
    Exchange, ExchangeAdapter, ExchangeConfig, ExchangeCapabilities, ExchangeStatus,
    ExchangeManager, KrakenAdapter, BinanceAdapter, CoinbaseAdapter,
    SymbolMapper, Symbol, NativeSymbol, create_adapter,
};

// Legacy retry exports
#[doc(hidden)]
pub use retry::{
    RetryPolicy, RetryPolicyBuilder, RetryExecutor, RetryableError,
    CircuitBreaker, CircuitState,
};

// Legacy middleware exports
#[doc(hidden)]
pub use middleware::{
    Middleware, MiddlewareChain, RequestContext, ResponseContext,
    LoggingMiddleware, MetricsMiddleware, RateLimitMiddleware,
    OperationMetrics, MetricsSnapshot,
};

// Legacy telemetry exports
#[doc(hidden)]
pub use telemetry::{
    TelemetryConfig, TelemetryConfigBuilder, MetricsRegistry,
    Counter, Gauge, Histogram, SdkMetrics, Span,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UTILITIES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use tracing_subscriber;

/// Initialize logging for the SDK
pub fn init_logging() {
    tracing_subscriber::fmt::init();
}
