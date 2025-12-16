//! Main client interface for the Kraken WebSocket SDK

use crate::{
    connection::{ConnectionManager, WebSocketMessage},
    data::*,
    error::{ProcessingError, SdkError},
    events::{EventCallback, EventDispatcher},
    orderbook::OrderBookManager,
    parser::{DataParser, KrakenDataParser, MessageHandler},
    subscription::SubscriptionManager,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

/// Main WebSocket client for Kraken API
pub struct KrakenWsClient {
    connection_manager: ConnectionManager,
    event_dispatcher: Arc<EventDispatcher>,
    subscription_manager: SubscriptionManager,
    orderbook_manager: OrderBookManager,
    message_handler: MessageHandler,
    config: ClientConfig,
    pending_subscriptions: Option<Vec<Channel>>,
}

impl KrakenWsClient {
    /// Create a new Kraken WebSocket client
    pub fn new(config: ClientConfig) -> Self {
        // Validate configuration
        if let Err(e) = config.validate() {
            tracing::error!("Invalid configuration: {}", e);
        }
        
        let connection_config = ConnectionConfig {
            endpoint: config.endpoint.clone(),
            timeout: config.timeout,
            ping_interval: std::time::Duration::from_secs(30),
        };
        
        let connection_manager = ConnectionManager::new(connection_config, config.reconnect_config.clone());
        let event_dispatcher = Arc::new(EventDispatcher::new());
        let subscription_manager = SubscriptionManager::new();
        let orderbook_manager = OrderBookManager::new();
        let parser = Arc::new(KrakenDataParser::new());
        let message_handler = MessageHandler::new(parser, Arc::clone(&event_dispatcher));
        
        Self {
            connection_manager,
            event_dispatcher,
            subscription_manager,
            orderbook_manager,
            message_handler,
            config,
            pending_subscriptions: None,
        }
    }
    
    /// Connect to the WebSocket API
    pub async fn connect(&mut self) -> Result<(), SdkError> {
        tracing::info!("Connecting to Kraken WebSocket API");
        
        let ws_stream = self.connection_manager.connect().await?;
        
        // Notify connection state change
        self.event_dispatcher.dispatch_connection_state_change(ConnectionState::Connected);
        
        // Start message processing loop
        self.start_message_loop(ws_stream).await?;
        
        Ok(())
    }
    
    /// Subscribe to market data channels
    pub async fn subscribe(&mut self, channels: Vec<Channel>) -> Result<(), SdkError> {
        tracing::info!("Subscribing to channels: {:?}", channels);
        
        // Validate channels and create subscription message
        let subscription_message = self.subscription_manager.create_subscription_message(&channels)?;
        
        // Store channels for later subscription after connection
        self.pending_subscriptions = Some(channels);
        
        tracing::info!("Subscription message prepared for {} channels", 
            self.pending_subscriptions.as_ref().map(|c| c.len()).unwrap_or(0));
        
        Ok(())
    }
    
    /// Unsubscribe from market data channels
    pub async fn unsubscribe(&mut self, channels: Vec<Channel>) -> Result<(), SdkError> {
        tracing::info!("Unsubscribing from channels: {:?}", channels);
        
        let unsubscription_message = self.subscription_manager.create_unsubscription_message(&channels)?;
        
        // TODO: Send unsubscription message through WebSocket connection
        tracing::info!("Unsubscription message created successfully for {} channels", channels.len());
        
        Ok(())
    }
    
    /// Register a callback for market data events
    pub fn register_callback(&self, data_type: DataType, callback: Arc<dyn EventCallback>) -> u64 {
        self.event_dispatcher.register_callback(data_type, callback)
    }
    
    /// Register a callback for connection state changes
    pub fn register_connection_listener(&self, callback: Arc<dyn EventCallback>) -> u64 {
        self.event_dispatcher.register_connection_listener(callback)
    }
    
    /// Get a unified event stream
    /// 
    /// Returns a receiver that will receive all SDK events (ticker, trade, book, ohlc, state, error).
    /// This is the recommended API for new code - simpler than callbacks and better for testing.
    /// 
    /// # Example
    /// ```rust,ignore
    /// let mut events = client.events();
    /// while let Some(event) = events.recv().await {
    ///     match event {
    ///         SdkEvent::Ticker(t) => println!("{}: {}", t.symbol, t.last_price),
    ///         SdkEvent::Trade(t) => println!("Trade: {} @ {}", t.symbol, t.price),
    ///         SdkEvent::State(s) => println!("Connection: {:?}", s),
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn events(&self) -> crate::events::EventReceiver {
        self.event_dispatcher.create_event_stream()
    }
    
    /// Disconnect from the WebSocket API
    pub async fn disconnect(&mut self) -> Result<(), SdkError> {
        tracing::info!("Disconnecting from Kraken WebSocket API");
        
        self.connection_manager.disconnect().await?;
        self.event_dispatcher.dispatch_connection_state_change(ConnectionState::Disconnected);
        
        Ok(())
    }
    
    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connection_manager.is_connected()
    }
    
    /// Get current connection state
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_manager.connection_state()
    }
    
    /// Start the message processing loop
    async fn start_message_loop(&mut self, mut ws_stream: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> Result<(), SdkError> {
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<Message>(100);
        
        // Send pending subscriptions if any
        if let Some(channels) = &self.pending_subscriptions {
            tracing::info!("Sending subscription for {} channels", channels.len());
            if let Ok(subscription_msg) = self.subscription_manager.create_subscription_message(channels) {
                if let Err(e) = ws_sender.send(subscription_msg).await {
                    tracing::error!("Failed to send subscription message: {}", e);
                } else {
                    tracing::info!("Subscription message sent successfully");
                }
            }
        }
        
        // Store sender for future messages
        let sender_clone = tx.clone();
        
        // Spawn task to handle outgoing messages
        let sender_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = ws_sender.send(message).await {
                    tracing::error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        });
        
        // Handle incoming messages
        let receiver_task = tokio::spawn({
            let message_handler = self.message_handler.clone();
            let event_dispatcher = Arc::clone(&self.event_dispatcher);
            let orderbook_manager = self.orderbook_manager.clone();
            
            async move {
                while let Some(message) = ws_receiver.next().await {
                    match message {
                        Ok(msg) => {
                            if let Err(e) = Self::handle_message_static(msg, &message_handler, &event_dispatcher, &orderbook_manager).await {
                                tracing::error!("Error handling message: {}", e);
                                event_dispatcher.dispatch_error(e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("WebSocket error: {}", e);
                            event_dispatcher.dispatch_error(SdkError::Network(format!("WebSocket error: {}", e)));
                            break;
                        }
                    }
                }
            }
        });
        
        // Wait for either task to complete
        tokio::select! {
            _ = sender_task => {
                tracing::info!("Sender task completed");
            }
            _ = receiver_task => {
                tracing::info!("Receiver task completed");
            }
        }
        
        Ok(())
    }
    
    /// Static method to handle messages (for use in async tasks)
    async fn handle_message_static(
        message: WebSocketMessage,
        message_handler: &MessageHandler,
        event_dispatcher: &Arc<EventDispatcher>,
        orderbook_manager: &OrderBookManager,
    ) -> Result<(), SdkError> {
        match message {
            Message::Text(text) => {
                tracing::debug!("Received message: {}", text);
                
                // Process the message
                if let Err(e) = message_handler.handle_message(&text).await {
                    tracing::warn!("Failed to process message, continuing: {}", e);
                }
                
                // Try to update order book if it's order book data
                if text.contains("book") {
                    if let Ok(parser) = KrakenDataParser::new().parse_orderbook(&text) {
                        if let Err(e) = orderbook_manager.apply_update(parser) {
                            tracing::warn!("Failed to update order book: {}", e);
                        }
                    }
                }
            }
            Message::Binary(data) => {
                tracing::debug!("Received binary message: {} bytes", data.len());
            }
            Message::Ping(data) => {
                tracing::debug!("Received ping, sending pong");
                // Connection manager should handle pong response
            }
            Message::Pong(_) => {
                tracing::debug!("Received pong");
            }
            Message::Close(_) => {
                tracing::info!("WebSocket connection closed");
                event_dispatcher.dispatch_connection_state_change(ConnectionState::Disconnected);
            }
            _ => {
                tracing::debug!("Received other message type");
            }
        }
        
        Ok(())
    }
    
    /// Handle incoming WebSocket messages
    async fn handle_message(&self, message: WebSocketMessage) -> Result<(), SdkError> {
        match message {
            Message::Text(text) => {
                tracing::debug!("Received message: {}", text);
                
                // Try to determine message type and parse accordingly
                if let Err(e) = self.process_message(&text).await {
                    // Handle malformed data gracefully - continue processing
                    tracing::warn!("Failed to process message, continuing: {}", e);
                }
            }
            Message::Binary(data) => {
                tracing::debug!("Received binary message: {} bytes", data.len());
                // Handle binary messages if needed
            }
            Message::Ping(_data) => {
                tracing::debug!("Received ping, sending pong");
                // Handle ping/pong for connection health
            }
            Message::Pong(_) => {
                tracing::debug!("Received pong");
            }
            Message::Close(_) => {
                tracing::info!("WebSocket connection closed");
                self.event_dispatcher.dispatch_connection_state_change(ConnectionState::Disconnected);
            }
            _ => {
                tracing::debug!("Received other message type");
            }
        }
        
        Ok(())
    }
    
    /// Process and route messages to appropriate parsers
    async fn process_message(&self, message: &str) -> Result<(), ProcessingError> {
        // Check if it's a subscription status message
        if message.contains("subscriptionStatus") {
            if let Err(e) = self.subscription_manager.handle_subscription_confirmation(message) {
                tracing::warn!("Subscription confirmation error: {}", e);
            }
            return Ok(());
        }
        
        // Use the message handler to process the message
        self.message_handler.handle_message(message).await
    }
    
    /// Handle reconnection logic
    async fn handle_reconnection(&self) -> Result<(), SdkError> {
        tracing::info!("Attempting to reconnect...");
        
        // This would trigger the reconnection logic in ConnectionManager
        // For now, we'll just log the attempt
        self.event_dispatcher.dispatch_connection_state_change(ConnectionState::Reconnecting);
        
        Ok(())
    }
    
    /// Get order book for a symbol
    pub fn get_order_book(&self, symbol: &str) -> Option<crate::orderbook::OrderBook> {
        self.orderbook_manager.get_order_book(symbol)
    }
    
    /// Get best bid and ask prices for a symbol
    pub fn get_best_bid_ask(&self, symbol: &str) -> Option<(Option<rust_decimal::Decimal>, Option<rust_decimal::Decimal>)> {
        self.orderbook_manager.get_best_bid_ask(symbol)
    }
    
    /// Get active subscriptions
    pub fn get_active_subscriptions(&self) -> Vec<String> {
        self.subscription_manager.get_active_subscriptions()
    }
    
    /// Check if subscribed to a channel
    pub fn is_subscribed(&self, channel: &Channel) -> bool {
        self.subscription_manager.is_subscribed(channel)
    }
    
    /// Get callback count for a data type
    pub fn get_callback_count(&self, data_type: &DataType) -> usize {
        self.event_dispatcher.get_callback_count(data_type)
    }
    
    /// Cleanup resources
    pub async fn cleanup(&mut self) -> Result<(), SdkError> {
        tracing::info!("Cleaning up SDK resources");
        
        // Disconnect if connected
        if self.is_connected() {
            self.disconnect().await?;
        }
        
        // Clear order books
        for symbol in self.orderbook_manager.get_symbols() {
            self.orderbook_manager.clear_order_book(&symbol);
        }
        
        tracing::info!("SDK cleanup completed");
        Ok(())
    }
}

/// Builder pattern for client configuration
pub struct ClientConfigBuilder {
    config: ClientConfig,
}

impl ClientConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }
    
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = endpoint.to_string();
        self
    }
    
    pub fn api_credentials(mut self, api_key: &str, api_secret: &str) -> Self {
        self.config.api_key = Some(api_key.to_string());
        self.config.api_secret = Some(api_secret.to_string());
        self
    }
    
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }
    
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.timeout = timeout;
        self
    }
    
    pub fn reconnect_config(mut self, reconnect_config: ReconnectConfig) -> Self {
        self.config.reconnect_config = reconnect_config;
        self
    }
    
    pub fn build(self) -> ClientConfig {
        self.config
    }
}

impl Default for ClientConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}