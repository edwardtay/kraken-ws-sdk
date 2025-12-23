//! Private WebSocket client for Kraken
//!
//! Provides authenticated WebSocket access for:
//! - Own trades (execution reports)
//! - Open orders (order status updates)
//! - Balances (real-time balance changes)

use crate::error::SdkError;
use crate::trading::{Execution, Order, OrderSide, OrderStatus, OrderType, Balances, AssetBalance};
use chrono::{DateTime, TimeZone, Utc};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const KRAKEN_WS_AUTH_URL: &str = "wss://ws-auth.kraken.com";

/// Private WebSocket event types
#[derive(Debug, Clone)]
pub enum PrivateEvent {
    /// Order execution (fill)
    Execution(Execution),
    /// Order status update
    OrderUpdate(OrderUpdate),
    /// Balance change
    BalanceUpdate(BalanceUpdate),
    /// Connection state change
    Connected,
    Disconnected,
    /// Error
    Error(String),
}

/// Order update from WebSocket
#[derive(Debug, Clone)]
pub struct OrderUpdate {
    pub txid: String,
    pub status: OrderStatus,
    pub volume_exec: Decimal,
    pub avg_price: Option<Decimal>,
    pub fee: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

/// Balance update from WebSocket
#[derive(Debug, Clone)]
pub struct BalanceUpdate {
    pub asset: String,
    pub balance: Decimal,
    pub available: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Private channel subscription
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateChannel {
    OwnTrades,
    OpenOrders,
}

impl PrivateChannel {
    fn as_str(&self) -> &'static str {
        match self {
            PrivateChannel::OwnTrades => "ownTrades",
            PrivateChannel::OpenOrders => "openOrders",
        }
    }
}

/// Configuration for private WebSocket client
#[derive(Debug, Clone)]
pub struct PrivateWsConfig {
    /// WebSocket authentication token (from REST API)
    pub token: String,
    /// Channels to subscribe to
    pub channels: Vec<PrivateChannel>,
    /// Reconnect on disconnect
    pub auto_reconnect: bool,
    /// Max reconnect attempts
    pub max_reconnect_attempts: u32,
}

impl PrivateWsConfig {
    pub fn new(token: String) -> Self {
        Self {
            token,
            channels: vec![PrivateChannel::OwnTrades, PrivateChannel::OpenOrders],
            auto_reconnect: true,
            max_reconnect_attempts: 10,
        }
    }

    pub fn with_channels(mut self, channels: Vec<PrivateChannel>) -> Self {
        self.channels = channels;
        self
    }
}

/// Private WebSocket client for authenticated feeds
pub struct PrivateWsClient {
    config: PrivateWsConfig,
    event_tx: broadcast::Sender<PrivateEvent>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    is_connected: Arc<RwLock<bool>>,
    // Track current state
    open_orders: Arc<RwLock<HashMap<String, Order>>>,
    recent_executions: Arc<RwLock<Vec<Execution>>>,
}

impl PrivateWsClient {
    /// Create a new private WebSocket client
    pub fn new(config: PrivateWsConfig) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        
        Self {
            config,
            event_tx,
            shutdown_tx: None,
            is_connected: Arc::new(RwLock::new(false)),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            recent_executions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<PrivateEvent> {
        self.event_tx.subscribe()
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// Get current open orders
    pub async fn get_open_orders(&self) -> Vec<Order> {
        self.open_orders.read().await.values().cloned().collect()
    }

    /// Get recent executions
    pub async fn get_recent_executions(&self) -> Vec<Execution> {
        self.recent_executions.read().await.clone()
    }

    /// Connect and start receiving events
    pub async fn connect(&mut self) -> Result<(), SdkError> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let is_connected = self.is_connected.clone();
        let open_orders = self.open_orders.clone();
        let recent_executions = self.recent_executions.clone();

        tokio::spawn(async move {
            let mut reconnect_attempts = 0;

            loop {
                match connect_and_run(
                    &config,
                    &event_tx,
                    &is_connected,
                    &open_orders,
                    &recent_executions,
                    &mut shutdown_rx,
                ).await {
                    Ok(()) => {
                        tracing::info!("Private WebSocket disconnected gracefully");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Private WebSocket error: {}", e);
                        let _ = event_tx.send(PrivateEvent::Error(e.to_string()));
                        
                        if !config.auto_reconnect {
                            break;
                        }

                        reconnect_attempts += 1;
                        if reconnect_attempts > config.max_reconnect_attempts {
                            tracing::error!("Max reconnect attempts reached");
                            break;
                        }

                        let delay = std::time::Duration::from_secs(2u64.pow(reconnect_attempts.min(5)));
                        tracing::info!("Reconnecting in {:?}...", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }

            *is_connected.write().await = false;
            let _ = event_tx.send(PrivateEvent::Disconnected);
        });

        // Wait for initial connection
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        
        if *self.is_connected.read().await {
            Ok(())
        } else {
            Err(SdkError::Connection(crate::error::ConnectionError::EstablishmentFailed(
                "Failed to establish private WebSocket connection".to_string()
            )))
        }
    }

    /// Disconnect
    pub async fn disconnect(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

async fn connect_and_run(
    config: &PrivateWsConfig,
    event_tx: &broadcast::Sender<PrivateEvent>,
    is_connected: &Arc<RwLock<bool>>,
    open_orders: &Arc<RwLock<HashMap<String, Order>>>,
    recent_executions: &Arc<RwLock<Vec<Execution>>>,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> Result<(), SdkError> {
    tracing::info!("Connecting to Kraken private WebSocket...");

    let (ws_stream, _) = connect_async(KRAKEN_WS_AUTH_URL)
        .await
        .map_err(|e| SdkError::Connection(crate::error::ConnectionError::EstablishmentFailed(e.to_string())))?;

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to channels
    for channel in &config.channels {
        let subscribe_msg = serde_json::json!({
            "event": "subscribe",
            "subscription": {
                "name": channel.as_str(),
                "token": config.token
            }
        });

        write.send(Message::Text(subscribe_msg.to_string()))
            .await
            .map_err(|e| SdkError::Network(e.to_string()))?;
    }

    *is_connected.write().await = true;
    let _ = event_tx.send(PrivateEvent::Connected);
    tracing::info!("Private WebSocket connected");

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                tracing::info!("Shutdown signal received");
                break;
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_message(&text, event_tx, open_orders, recent_executions).await {
                            tracing::warn!("Error handling message: {}", e);
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        tracing::info!("WebSocket closed by server");
                        break;
                    }
                    Some(Err(e)) => {
                        return Err(SdkError::Network(e.to_string()));
                    }
                    None => {
                        tracing::info!("WebSocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    *is_connected.write().await = false;
    Ok(())
}

async fn handle_message(
    text: &str,
    event_tx: &broadcast::Sender<PrivateEvent>,
    open_orders: &Arc<RwLock<HashMap<String, Order>>>,
    recent_executions: &Arc<RwLock<Vec<Execution>>>,
) -> Result<(), SdkError> {
    let json: Value = serde_json::from_str(text)
        .map_err(|e| SdkError::Parse(crate::error::ParseError::InvalidJson(e.to_string())))?;

    // Handle system messages
    if let Some(event) = json.get("event").and_then(|e| e.as_str()) {
        match event {
            "systemStatus" | "subscriptionStatus" | "heartbeat" => {
                tracing::debug!("System message: {}", event);
                return Ok(());
            }
            "error" => {
                let error_msg = json["errorMessage"].as_str().unwrap_or("Unknown error");
                tracing::error!("WebSocket error: {}", error_msg);
                let _ = event_tx.send(PrivateEvent::Error(error_msg.to_string()));
                return Ok(());
            }
            _ => {}
        }
    }

    // Handle data messages (arrays)
    if let Some(arr) = json.as_array() {
        if arr.len() >= 2 {
            let channel_name = arr.last().and_then(|v| v.as_str()).unwrap_or("");
            
            match channel_name {
                "ownTrades" => {
                    handle_own_trades(&arr[0], event_tx, recent_executions).await?;
                }
                "openOrders" => {
                    handle_open_orders(&arr[0], event_tx, open_orders).await?;
                }
                _ => {
                    tracing::debug!("Unknown channel: {}", channel_name);
                }
            }
        }
    }

    Ok(())
}

async fn handle_own_trades(
    data: &Value,
    event_tx: &broadcast::Sender<PrivateEvent>,
    recent_executions: &Arc<RwLock<Vec<Execution>>>,
) -> Result<(), SdkError> {
    if let Some(trades) = data.as_array() {
        for trade_obj in trades {
            if let Some(trade_map) = trade_obj.as_object() {
                for (trade_id, trade_data) in trade_map {
                    let execution = Execution {
                        trade_id: trade_id.clone(),
                        order_txid: trade_data["ordertxid"].as_str().unwrap_or("").to_string(),
                        pair: trade_data["pair"].as_str().unwrap_or("").to_string(),
                        side: if trade_data["type"].as_str() == Some("buy") { OrderSide::Buy } else { OrderSide::Sell },
                        order_type: OrderType::Limit,
                        price: parse_decimal_str(trade_data["price"].as_str()),
                        volume: parse_decimal_str(trade_data["vol"].as_str()),
                        cost: parse_decimal_str(trade_data["cost"].as_str()),
                        fee: parse_decimal_str(trade_data["fee"].as_str()),
                        fee_currency: "USD".to_string(),
                        time: parse_timestamp_str(trade_data["time"].as_str()),
                    };

                    // Store execution
                    {
                        let mut execs = recent_executions.write().await;
                        execs.push(execution.clone());
                        // Keep last 100 executions
                        if execs.len() > 100 {
                            execs.remove(0);
                        }
                    }

                    let _ = event_tx.send(PrivateEvent::Execution(execution));
                }
            }
        }
    }
    Ok(())
}

async fn handle_open_orders(
    data: &Value,
    event_tx: &broadcast::Sender<PrivateEvent>,
    open_orders: &Arc<RwLock<HashMap<String, Order>>>,
) -> Result<(), SdkError> {
    if let Some(orders) = data.as_array() {
        for order_obj in orders {
            if let Some(order_map) = order_obj.as_object() {
                for (txid, order_data) in order_map {
                    let status_str = order_data["status"].as_str().unwrap_or("open");
                    let status = match status_str {
                        "pending" => OrderStatus::Pending,
                        "open" => OrderStatus::Open,
                        "closed" => OrderStatus::Closed,
                        "canceled" => OrderStatus::Canceled,
                        "expired" => OrderStatus::Expired,
                        _ => OrderStatus::Open,
                    };

                    let update = OrderUpdate {
                        txid: txid.clone(),
                        status,
                        volume_exec: parse_decimal_str(order_data["vol_exec"].as_str()),
                        avg_price: order_data["avg_price"].as_str().and_then(|s| s.parse().ok()),
                        fee: order_data["fee"].as_str().and_then(|s| s.parse().ok()),
                        timestamp: Utc::now(),
                    };

                    // Update order tracking
                    {
                        let mut orders = open_orders.write().await;
                        if matches!(status, OrderStatus::Closed | OrderStatus::Canceled | OrderStatus::Expired) {
                            orders.remove(txid);
                        }
                        // Note: For new orders, we'd need full order data which isn't always in the update
                    }

                    let _ = event_tx.send(PrivateEvent::OrderUpdate(update));
                }
            }
        }
    }
    Ok(())
}

fn parse_decimal_str(s: Option<&str>) -> Decimal {
    s.and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO)
}

fn parse_timestamp_str(s: Option<&str>) -> DateTime<Utc> {
    s.and_then(|s| s.parse::<f64>().ok())
        .map(|t| Utc.timestamp_opt(t as i64, ((t.fract()) * 1_000_000_000.0) as u32).unwrap())
        .unwrap_or_else(Utc::now)
}

impl Drop for PrivateWsClient {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}
