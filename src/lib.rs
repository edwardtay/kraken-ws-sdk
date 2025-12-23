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
// TRADING API - For authenticated trading operations
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Trading API for authenticated operations
///
/// This module provides everything needed for trading on Kraken:
/// - REST API client for orders, balances, positions
/// - Private WebSocket for real-time execution reports
/// - Rate limiting and authentication
///
/// ## Example
///
/// ```rust,ignore
/// use kraken_ws_sdk::trading_api::*;
///
/// // Create REST client from environment
/// let client = KrakenRestClient::from_env()?;
///
/// // Get balances
/// let balances = client.get_balance().await?;
/// println!("BTC: {}", balances.available("XXBT"));
///
/// // Place a limit order
/// let order = OrderRequest::limit_buy("XBT/USD", dec!(0.001), dec!(50000.00))
///     .post_only()
///     .with_client_id("my-order-1");
/// let response = client.add_order(order).await?;
/// println!("Order placed: {:?}", response.txid);
///
/// // Subscribe to execution reports
/// let token = client.get_websocket_token().await?;
/// let mut ws = PrivateWsClient::new(PrivateWsConfig::new(token));
/// ws.connect().await?;
///
/// let mut events = ws.subscribe();
/// while let Ok(event) = events.recv().await {
///     match event {
///         PrivateEvent::Execution(exec) => {
///             println!("Fill: {} {} @ {}", exec.volume, exec.pair, exec.price);
///         }
///         PrivateEvent::OrderUpdate(update) => {
///             println!("Order {}: {:?}", update.txid, update.status);
///         }
///         _ => {}
///     }
/// }
/// ```
pub mod trading_api {
    // Authentication
    pub use crate::auth::Credentials;
    
    // Rate limiting
    pub use crate::rate_limit::{RateLimiter, AccountTier, EndpointCost, RateLimitStats};
    
    // REST client
    pub use crate::rest_client::{
        KrakenRestClient, TradesHistoryOptions, ClosedOrdersOptions,
    };
    
    // Trading types
    pub use crate::trading::{
        OrderSide, OrderType, TimeInForce, OrderFlags,
        OrderRequest, OrderResponse, OrderDescription,
        OrderStatus, Order, Execution,
        CancelRequest, CancelResponse, EditOrderRequest,
        AssetBalance, Balances, Position,
    };
    
    // Private WebSocket
    pub use crate::private_ws::{
        PrivateWsClient, PrivateWsConfig, PrivateChannel,
        PrivateEvent, OrderUpdate, BalanceUpdate,
    };
    
    // Batch orders & advanced order types
    pub use crate::batch_orders::{
        BatchOrderRequest, BatchOrderResult, BatchOrderError,
        OcoOrder, OcoOrderResult,
        BracketOrder, BracketOrderResult,
        sizing,
    };
    
    // Performance tracking
    pub use crate::performance::{
        PerformanceTracker, PerformanceStats,
        CompletedTrade, EquityPoint,
    };
    
    // Alerts
    pub use crate::alerts::{
        AlertManager, Alert, AlertType, AlertSeverity,
        AlertChannel, ConsoleChannel, WebhookChannel,
        price_alert, order_filled, pnl_alert, risk_alert,
    };
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
    
    // Order book visualization
    pub use crate::orderbook::{
        AggregatedBook, AggregatedLevel, DepthLadder, LadderLevel,
        ImbalanceMetrics, BookPressure, PressureSignal,
    };
    
    // Order flow tracking
    pub use crate::orderflow::{
        OrderFlowTracker, OrderFlowConfig, FlowEvent, FlowEventType, FlowSide,
        TradesByPriceLevel, TradeOverlayConfig, LevelTrade, LevelTradeStats,
        MarketHealthTracker, MarketStatus, StaleDetectionConfig,
    };
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VISUALIZATION API - For building order book UIs
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Visualization API for building professional order book interfaces
///
/// This module provides everything needed to build a professional-grade
/// order book visualization with:
/// - Real-time depth ladders with cumulative sizes
/// - Price aggregation (tick size grouping)
/// - Liquidity imbalance indicators
/// - Order flow highlighting (large order detection)
/// - Recent trades overlay by price level
/// - Market health/stale detection
///
/// ## Example
///
/// ```rust,ignore
/// use kraken_ws_sdk::visualization::*;
/// use rust_decimal_macros::dec;
///
/// // Set up order flow tracking
/// let flow_tracker = OrderFlowTracker::with_config(OrderFlowConfig {
///     large_order_threshold: dec!(10.0),
///     track_depth: 25,
///     ..Default::default()
/// });
///
/// // Set up trade overlay
/// let trade_tracker = TradesByPriceLevel::new();
///
/// // Set up market health monitoring
/// let health_tracker = MarketHealthTracker::new();
///
/// // On each order book update:
/// let ladder = order_book.get_depth_ladder(20);
/// let flow_events = flow_tracker.track_update(&order_book);
/// let imbalance = order_book.get_imbalance_ratio(10);
/// health_tracker.record_update(&order_book.symbol);
///
/// // Render your UI with this data
/// ```
pub mod visualization {
    // Order book structures
    pub use crate::orderbook::{
        OrderBook, OrderBookManager,
        AggregatedBook, AggregatedLevel,
        DepthLadder, LadderLevel,
        ImbalanceMetrics, BookPressure, PressureSignal,
        FilteredBook,  // Depth range selector
    };
    
    // Order flow tracking
    pub use crate::orderflow::{
        OrderFlowTracker, OrderFlowConfig,
        FlowEvent, FlowEventType, FlowSide,
    };
    
    // Trade overlay
    pub use crate::orderflow::{
        TradesByPriceLevel, TradeOverlayConfig,
        LevelTrade, LevelTradeStats,
    };
    
    // Market health
    pub use crate::orderflow::{
        MarketHealthTracker, MarketStatus, StaleDetectionConfig,
    };
    
    // Data types needed for visualization
    pub use crate::data::{PriceLevel, TradeData, TradeSide};
    
    // Latency for UI indicators
    pub use crate::latency::{LatencyTracker, LatencyStats, format_latency};
    
    /// Advanced visualization features for professional trading UIs
    ///
    /// These are opt-in features that provide edge for power users
    /// without cluttering the basic API.
    pub mod advanced {
        // Whale detection (statistical outliers)
        pub use crate::whale_detection::{
            WhaleDetector, WhaleConfig,
            WhaleDetection, WhaleSide,
        };
        
        // Liquidity heatmap (persistence tracking)
        pub use crate::liquidity_heatmap::{
            LiquidityHeatmap, HeatmapConfig,
            HeatLevel, HeatmapSnapshot, HeatmapSide,
        };
        
        // Spoofing detection (vanishing liquidity)
        pub use crate::spoofing_detection::{
            SpoofingDetector, SpoofingConfig,
            SpoofingAlert, SpoofSide,
        };
    }
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
pub mod orderflow;  // Order flow tracking for visualization
pub mod whale_detection;  // Statistical whale order detection
pub mod liquidity_heatmap;  // Liquidity persistence tracking
pub mod spoofing_detection;  // Spoofing pattern detection
pub mod parser;
pub mod retry;
pub mod sdk;
pub mod sequencing;
pub mod state;  // Connection state machine
pub mod subscription;
pub mod telemetry;

// Private/authenticated API modules
pub mod auth;           // API key authentication & request signing
pub mod rate_limit;     // Kraken rate limiting
pub mod trading;        // Order types, positions, balances
pub mod rest_client;    // REST API client
pub mod private_ws;     // Private WebSocket channels

// Advanced trading features
pub mod batch_orders;   // Batch orders, OCO, bracket orders
pub mod performance;    // Performance tracking (P&L, Sharpe, drawdown)
pub mod alerts;         // Alert system (webhook, Discord, Telegram)

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
