//! Orderbook state management and time-travel functionality

use crate::storage::{OrderbookSnapshot, OrderbookStorage, PriceLevel};
use chrono::{DateTime, Utc};
use kraken_ws_sdk::{Channel, ClientConfig, DataType, EventCallback, KrakenWsClient, OrderBookUpdate, ConnectionState, SdkError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

/// Orderbook manager with real-time updates and time-travel
pub struct OrderbookManager {
    storage: Arc<OrderbookStorage>,
    current_books: Arc<Mutex<HashMap<String, OrderbookSnapshot>>>,
    update_tx: broadcast::Sender<OrderbookSnapshot>,
}

impl OrderbookManager {
    /// Create a new orderbook manager
    pub fn new(storage_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let storage = Arc::new(OrderbookStorage::new(storage_path)?);
        let current_books = Arc::new(Mutex::new(HashMap::new()));
        let (update_tx, _) = broadcast::channel(1000);

        Ok(Self {
            storage,
            current_books,
            update_tx,
        })
    }

    /// Get the current orderbook for a symbol
    pub fn get_current(&self, symbol: &str) -> Option<OrderbookSnapshot> {
        self.current_books.lock().unwrap().get(symbol).cloned()
    }

    /// Get orderbook history
    pub fn get_history(
        &self,
        symbol: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<OrderbookSnapshot>, Box<dyn std::error::Error>> {
        self.storage.get_range(symbol, from, to)
    }

    /// Get snapshot at specific time
    pub fn get_at_time(
        &self,
        symbol: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<OrderbookSnapshot>, Box<dyn std::error::Error>> {
        // Try to find exact match first
        if let Some(snapshot) = self.storage.get_at_time(symbol, timestamp)? {
            return Ok(Some(snapshot));
        }

        // Otherwise, find the closest snapshot before the requested time
        let from = timestamp - chrono::Duration::hours(1);
        let snapshots = self.storage.get_range(symbol, from, timestamp)?;

        Ok(snapshots.last().cloned())
    }

    /// Subscribe to real-time updates
    pub fn subscribe_updates(&self) -> broadcast::Receiver<OrderbookSnapshot> {
        self.update_tx.subscribe()
    }

    /// Update orderbook state (called from WebSocket callback)
    pub fn update_orderbook(&self, update: OrderBookUpdate) {
        let snapshot = OrderbookSnapshot {
            symbol: update.symbol.clone(),
            timestamp: update.timestamp,
            bids: update.bids.iter().map(|level| PriceLevel {
                price: level.price,
                volume: level.volume,
                order_count: None,
            }).collect(),
            asks: update.asks.iter().map(|level| PriceLevel {
                price: level.price,
                volume: level.volume,
                order_count: None,
            }).collect(),
            checksum: update.checksum,
            sequence: None,
        };

        // Update current state
        {
            let mut current = self.current_books.lock().unwrap();
            current.insert(update.symbol.clone(), snapshot.clone());
        }

        // Store snapshot
        if let Err(e) = self.storage.store_snapshot(&snapshot) {
            tracing::error!("Failed to store snapshot: {}", e);
        }

        // Broadcast update
        let _ = self.update_tx.send(snapshot);
    }

    /// Get storage for a symbol
    pub fn get_stats(&self, symbol: &str) -> Result<crate::storage::StorageStats, Box<dyn std::error::Error>> {
        self.storage.get_stats(symbol)
    }
}

/// Kraken WebSocket callback that feeds the orderbook manager
pub struct OrderbookCallback {
    manager: Arc<OrderbookManager>,
}

impl OrderbookCallback {
    pub fn new(manager: Arc<OrderbookManager>) -> Self {
        Self { manager }
    }
}

impl EventCallback for OrderbookCallback {
    fn on_orderbook(&self, data: OrderBookUpdate) {
        self.manager.update_orderbook(data);
    }

    fn on_connection_state_change(&self, state: ConnectionState) {
        tracing::info!("Connection state changed: {:?}", state);
    }

    fn on_error(&self, error: SdkError) {
        tracing::error!("SDK error: {}", error);
    }

    // Implement other required methods as no-ops
    fn on_ticker(&self, _data: kraken_ws_sdk::TickerData) {}
    fn on_trade(&self, _data: kraken_ws_sdk::TradeData) {}
    fn on_ohlc(&self, _data: kraken_ws_sdk::OHLCData) {}
}

/// Start Kraken WebSocket client for orderbook data
pub async fn start_kraken_client(
    manager: Arc<OrderbookManager>,
    symbols: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig {
        endpoint: "wss://ws.kraken.com".to_string(),
        timeout: Duration::from_secs(30),
        buffer_size: 4096,
        ..Default::default()
    };

    let mut client = KrakenWsClient::new(config);
    let callback = Arc::new(OrderbookCallback::new(manager));

    client.register_callback(DataType::OrderBook, callback.clone());
    client.register_connection_listener(callback);

    // Subscribe to orderbook channels
    let channels: Vec<Channel> = symbols
        .iter()
        .map(|symbol| Channel::new("book").with_symbol(symbol).with_depth(10))
        .collect();

    client.subscribe(channels).await?;
    client.connect().await?;

    tracing::info!("Connected to Kraken WebSocket API");

    // Keep connection alive
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        tracing::debug!("WebSocket connection heartbeat");
    }
}
