//! WebSocket connection management

use crate::{
    data::{ConnectionConfig, ConnectionState, ReconnectConfig},
    error::ConnectionError,
};
// use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use url::Url;

/// WebSocket message type
pub type WebSocketMessage = Message;

/// Connection manager for WebSocket connections
pub struct ConnectionManager {
    config: ConnectionConfig,
    state: Arc<Mutex<ConnectionState>>,
    reconnect_strategy: ReconnectStrategy,
    last_ping: Arc<Mutex<Option<Instant>>>,
    last_pong: Arc<Mutex<Option<Instant>>>,
}

impl ConnectionManager {
    pub fn new(config: ConnectionConfig, reconnect_config: ReconnectConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            reconnect_strategy: ReconnectStrategy::new(reconnect_config),
            last_ping: Arc::new(Mutex::new(None)),
            last_pong: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Establish WebSocket connection
    pub async fn connect(&mut self) -> Result<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, ConnectionError> {
        self.set_state(ConnectionState::Connecting);
        
        let url = Url::parse(&self.config.endpoint)
            .map_err(|e| ConnectionError::EstablishmentFailed(format!("Invalid URL: {}", e)))?;
        
        let connect_future = connect_async(url);
        let timeout_future = sleep(self.config.timeout);
        
        tokio::select! {
            result = connect_future => {
                match result {
                    Ok((ws_stream, _)) => {
                        self.set_state(ConnectionState::Connected);
                        self.reconnect_strategy.reset();
                        tracing::info!("WebSocket connection established");
                        Ok(ws_stream)
                    }
                    Err(e) => {
                        self.set_state(ConnectionState::Failed);
                        Err(ConnectionError::EstablishmentFailed(format!("Connection failed: {}", e)))
                    }
                }
            }
            _ = timeout_future => {
                self.set_state(ConnectionState::Failed);
                Err(ConnectionError::Timeout("Connection timeout".to_string()))
            }
        }
    }
    
    /// Disconnect WebSocket connection
    pub async fn disconnect(&mut self) -> Result<(), ConnectionError> {
        self.set_state(ConnectionState::Disconnected);
        tracing::info!("WebSocket connection disconnected");
        Ok(())
    }
    
    /// Check if connection is active
    pub fn is_connected(&self) -> bool {
        matches!(*self.state.lock().unwrap(), ConnectionState::Connected)
    }
    
    /// Get current connection state
    pub fn connection_state(&self) -> ConnectionState {
        self.state.lock().unwrap().clone()
    }
    
    /// Attempt reconnection with exponential backoff
    pub async fn reconnect(&mut self) -> Result<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, ConnectionError> {
        self.set_state(ConnectionState::Reconnecting);
        tracing::info!("Starting reconnection process with exponential backoff");
        
        for attempt in 1..=self.reconnect_strategy.config.max_attempts {
            let delay = if attempt > 1 { 
                Some(self.reconnect_strategy.next_delay()) 
            } else { 
                None 
            };
            
            if let Some(delay) = delay {
                tracing::info!("Waiting {:?} before reconnection attempt {} of {}", 
                    delay, attempt, self.reconnect_strategy.config.max_attempts);
                sleep(delay).await;
            }
            
            tracing::info!("Reconnection attempt {} of {}", attempt, self.reconnect_strategy.config.max_attempts);
            
            match self.connect().await {
                Ok(stream) => {
                    tracing::info!("Reconnection successful after {} attempts", attempt);
                    return Ok(stream);
                }
                Err(e) => {
                    tracing::warn!("Reconnection attempt {} failed: {}", attempt, e);
                    
                    // Classify error type for better handling
                    match &e {
                        ConnectionError::Timeout(_) => {
                            tracing::debug!("Timeout error - will retry");
                        }
                        ConnectionError::EstablishmentFailed(_) => {
                            tracing::debug!("Network error - will retry");
                        }
                        ConnectionError::AuthenticationFailed(_) => {
                            tracing::error!("Authentication failed - stopping reconnection attempts");
                            self.set_state(ConnectionState::Failed);
                            return Err(e);
                        }
                        _ => {}
                    }
                }
            }
        }
        
        self.set_state(ConnectionState::Failed);
        tracing::error!("All {} reconnection attempts failed", self.reconnect_strategy.config.max_attempts);
        Err(ConnectionError::EstablishmentFailed("All reconnection attempts failed".to_string()))
    }
    
    /// Handle authentication for private channels
    pub async fn authenticate(&self, api_key: &str, api_secret: &str) -> Result<Message, ConnectionError> {
        if api_key.is_empty() || api_secret.is_empty() {
            return Err(ConnectionError::AuthenticationFailed(
                "API key and secret are required for authentication".to_string()
            ));
        }
        
        // Generate nonce (timestamp in milliseconds)
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        // Create authentication message according to Kraken's WebSocket protocol
        let auth_message = serde_json::json!({
            "event": "subscribe",
            "subscription": {
                "name": "ownTrades",
                "token": self.generate_auth_token(api_key, api_secret, nonce)?
            },
            "reqid": uuid::Uuid::new_v4().to_string()
        });
        
        let message_str = serde_json::to_string(&auth_message)
            .map_err(|e| ConnectionError::AuthenticationFailed(format!("Failed to serialize auth message: {}", e)))?;
        
        tracing::info!("Authentication message prepared for API key: {}...", &api_key[..std::cmp::min(8, api_key.len())]);
        Ok(Message::Text(message_str))
    }
    
    /// Generate authentication token for Kraken WebSocket API
    fn generate_auth_token(&self, api_key: &str, _api_secret: &str, nonce: u64) -> Result<String, ConnectionError> {
        // For now, return a placeholder token
        // In a real implementation, this would generate a proper HMAC signature
        // according to Kraken's authentication specification
        
        // Base64 encode the token (simplified implementation)
        use std::collections::HashMap;
        let mut token_data = HashMap::new();
        let nonce_str = nonce.to_string();
        token_data.insert("api_key", api_key);
        token_data.insert("nonce", nonce_str.as_str());
        
        // In production, this would be a proper HMAC-SHA512 signature
        let token_json = serde_json::to_string(&token_data)
            .map_err(|e| ConnectionError::AuthenticationFailed(format!("Token generation failed: {}", e)))?;
        
        use base64::{Engine as _, engine::general_purpose};
        Ok(general_purpose::STANDARD.encode(token_json))
    }
    
    /// Validate authentication response
    pub fn validate_auth_response(&self, response: &str) -> Result<bool, ConnectionError> {
        let json: serde_json::Value = serde_json::from_str(response)
            .map_err(|e| ConnectionError::AuthenticationFailed(format!("Invalid auth response: {}", e)))?;
        
        if let Some(event) = json.get("event").and_then(|v| v.as_str()) {
            if event == "subscriptionStatus" {
                if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                    match status {
                        "subscribed" => {
                            tracing::info!("Authentication successful");
                            return Ok(true);
                        }
                        "error" => {
                            let error_msg = json.get("errorMessage")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Authentication failed");
                            return Err(ConnectionError::AuthenticationFailed(error_msg.to_string()));
                        }
                        _ => {}
                    }
                }
            }
        }
        
        Ok(false)
    }
    
    fn set_state(&self, state: ConnectionState) {
        *self.state.lock().unwrap() = state;
    }
    
    /// Update ping timestamp
    pub fn update_ping(&self) {
        *self.last_ping.lock().unwrap() = Some(Instant::now());
    }
    
    /// Update pong timestamp
    pub fn update_pong(&self) {
        *self.last_pong.lock().unwrap() = Some(Instant::now());
    }
    
    /// Check connection health based on ping/pong timing
    pub fn is_healthy(&self) -> bool {
        let ping = self.last_ping.lock().unwrap();
        let pong = self.last_pong.lock().unwrap();
        
        match (*ping, *pong) {
            (Some(ping_time), Some(pong_time)) => {
                // Connection is healthy if pong was received after ping
                // and within reasonable time frame
                pong_time >= ping_time && 
                ping_time.elapsed() < Duration::from_secs(60)
            }
            (None, None) => true, // No pings sent yet, assume healthy
            _ => false, // Ping sent but no pong received
        }
    }
}

/// Reconnection strategy with exponential backoff
pub struct ReconnectStrategy {
    config: ReconnectConfig,
    current_delay: Duration,
    last_attempt: Option<Instant>,
}

impl ReconnectStrategy {
    pub fn new(config: ReconnectConfig) -> Self {
        Self {
            current_delay: config.initial_delay,
            config,
            last_attempt: None,
        }
    }
    
    /// Get next delay duration with exponential backoff
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current_delay;
        
        // Apply exponential backoff
        let next_delay_ms = (self.current_delay.as_millis() as f64 * self.config.backoff_multiplier) as u64;
        let next_delay = Duration::from_millis(next_delay_ms);
        
        // Cap at maximum delay
        self.current_delay = std::cmp::min(next_delay, self.config.max_delay);
        self.last_attempt = Some(Instant::now());
        
        tracing::debug!("Exponential backoff: current delay = {:?}, next delay = {:?}", 
            delay, self.current_delay);
        
        delay
    }
    
    /// Reset reconnection strategy
    pub fn reset(&mut self) {
        self.current_delay = self.config.initial_delay;
        self.last_attempt = None;
    }
}