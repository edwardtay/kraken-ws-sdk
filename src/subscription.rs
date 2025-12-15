//! Subscription management for WebSocket channels

use crate::{
    data::Channel,
    error::SubscriptionError,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio_tungstenite::tungstenite::Message;

/// Subscription manager for handling channel subscriptions
#[derive(Debug)]
pub struct SubscriptionManager {
    active_subscriptions: Arc<Mutex<HashSet<String>>>,
    pending_subscriptions: Arc<Mutex<HashMap<String, Channel>>>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            active_subscriptions: Arc::new(Mutex::new(HashSet::new())),
            pending_subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Create subscription message for a channel
    pub fn create_subscription_message(&self, channels: &[Channel]) -> Result<Message, SubscriptionError> {
        if channels.is_empty() {
            return Err(SubscriptionError::InvalidChannel("No channels provided".to_string()));
        }
        
        // Validate channels
        for channel in channels {
            self.validate_channel(channel)?;
        }
        
        // Group channels by subscription type
        let mut ticker_pairs = Vec::new();
        let mut trade_pairs = Vec::new();
        
        for channel in channels {
            if let Some(symbol) = &channel.symbol {
                match channel.name.as_str() {
                    "ticker" => ticker_pairs.push(symbol.clone()),
                    "trade" => trade_pairs.push(symbol.clone()),
                    _ => {}
                }
            }
        }
        
        // Create subscription message for ticker data (most common case)
        let (pairs, subscription_name) = if !ticker_pairs.is_empty() {
            (ticker_pairs, "ticker")
        } else if !trade_pairs.is_empty() {
            (trade_pairs, "trade")
        } else {
            return Err(SubscriptionError::InvalidChannel("No valid pairs found".to_string()));
        };
        
        let message = json!({
            "event": "subscribe",
            "pair": pairs,
            "subscription": {
                "name": subscription_name
            }
        });
        
        let message_str = serde_json::to_string(&message)
            .map_err(|e| SubscriptionError::SubscriptionFailed(format!("Failed to serialize subscription message: {}", e)))?;
        
        // Add to pending subscriptions
        for channel in channels {
            let subscription_key = self.generate_subscription_key(channel);
            let mut pending = self.pending_subscriptions.lock().unwrap();
            pending.insert(subscription_key, channel.clone());
        }
        
        tracing::info!("Created subscription message: {}", message_str);
        Ok(Message::Text(message_str))
    }
    
    /// Create unsubscription message for a channel
    pub fn create_unsubscription_message(&self, channels: &[Channel]) -> Result<Message, SubscriptionError> {
        if channels.is_empty() {
            return Err(SubscriptionError::InvalidChannel("No channels provided".to_string()));
        }
        
        // Check if subscribed
        let subscription_key = self.generate_subscription_key(&channels[0]);
        {
            let active = self.active_subscriptions.lock().unwrap();
            if !active.contains(&subscription_key) {
                return Err(SubscriptionError::NotSubscribed(format!("Not subscribed to channel: {}", channels[0].name)));
            }
        }
        
        let mut subscription_data = json!({
            "name": channels[0].name
        });
        
        if let Some(symbol) = &channels[0].symbol {
            subscription_data["symbol"] = json!(symbol);
        }
        
        if let Some(interval) = &channels[0].interval {
            subscription_data["interval"] = json!(interval);
        }
        
        let message = json!({
            "event": "unsubscribe",
            "pair": channels.iter()
                .filter_map(|c| c.symbol.as_ref())
                .collect::<Vec<_>>(),
            "subscription": subscription_data,
            "reqid": uuid::Uuid::new_v4().to_string()
        });
        
        let message_str = serde_json::to_string(&message)
            .map_err(|e| SubscriptionError::SubscriptionFailed(format!("Failed to serialize unsubscription message: {}", e)))?;
        
        Ok(Message::Text(message_str))
    }
    
    /// Handle subscription confirmation
    pub fn handle_subscription_confirmation(&self, message: &str) -> Result<(), SubscriptionError> {
        let json: Value = serde_json::from_str(message)
            .map_err(|e| SubscriptionError::SubscriptionFailed(format!("Invalid confirmation message: {}", e)))?;
        
        if let Some(event) = json.get("event").and_then(|v| v.as_str()) {
            if event == "subscriptionStatus" {
                if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                    if status == "subscribed" {
                        // Move from pending to active
                        if let Some(subscription) = json.get("subscription") {
                            if let Some(name) = subscription.get("name").and_then(|v| v.as_str()) {
                                let channel = Channel::new(name);
                                let subscription_key = self.generate_subscription_key(&channel);
                                
                                {
                                    let mut pending = self.pending_subscriptions.lock().unwrap();
                                    pending.remove(&subscription_key);
                                }
                                
                                {
                                    let mut active = self.active_subscriptions.lock().unwrap();
                                    active.insert(subscription_key);
                                }
                                
                                tracing::info!("Subscription confirmed for channel: {}", name);
                                return Ok(());
                            }
                        }
                    } else if status == "error" {
                        let error_msg = json.get("errorMessage")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown subscription error");
                        return Err(SubscriptionError::SubscriptionFailed(error_msg.to_string()));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle unsubscription confirmation
    pub fn handle_unsubscription_confirmation(&self, message: &str) -> Result<(), SubscriptionError> {
        let json: Value = serde_json::from_str(message)
            .map_err(|e| SubscriptionError::SubscriptionFailed(format!("Invalid confirmation message: {}", e)))?;
        
        if let Some(event) = json.get("event").and_then(|v| v.as_str()) {
            if event == "subscriptionStatus" {
                if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                    if status == "unsubscribed" {
                        if let Some(subscription) = json.get("subscription") {
                            if let Some(name) = subscription.get("name").and_then(|v| v.as_str()) {
                                let channel = Channel::new(name);
                                let subscription_key = self.generate_subscription_key(&channel);
                                
                                {
                                    let mut active = self.active_subscriptions.lock().unwrap();
                                    active.remove(&subscription_key);
                                }
                                
                                tracing::info!("Unsubscription confirmed for channel: {}", name);
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if subscribed to a channel
    pub fn is_subscribed(&self, channel: &Channel) -> bool {
        let subscription_key = self.generate_subscription_key(channel);
        let active = self.active_subscriptions.lock().unwrap();
        active.contains(&subscription_key)
    }
    
    /// Get list of active subscriptions
    pub fn get_active_subscriptions(&self) -> Vec<String> {
        let active = self.active_subscriptions.lock().unwrap();
        active.iter().cloned().collect()
    }
    
    /// Validate channel specification
    fn validate_channel(&self, channel: &Channel) -> Result<(), SubscriptionError> {
        // Validate channel name
        let valid_channels = ["ticker", "ohlc", "trade", "book", "spread"];
        if !valid_channels.contains(&channel.name.as_str()) {
            return Err(SubscriptionError::InvalidChannel(
                format!("Invalid channel name: {}. Valid channels: {:?}", channel.name, valid_channels)
            ));
        }
        
        // Validate interval for OHLC
        if channel.name == "ohlc" {
            if let Some(interval) = &channel.interval {
                let valid_intervals = ["1", "5", "15", "30", "60", "240", "1440", "10080", "21600"];
                if !valid_intervals.contains(&interval.as_str()) {
                    return Err(SubscriptionError::InvalidChannel(
                        format!("Invalid OHLC interval: {}. Valid intervals: {:?}", interval, valid_intervals)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Generate unique subscription key
    fn generate_subscription_key(&self, channel: &Channel) -> String {
        let mut key = channel.name.clone();
        if let Some(symbol) = &channel.symbol {
            key.push_str(&format!(":{}", symbol));
        }
        if let Some(interval) = &channel.interval {
            key.push_str(&format!(":{}", interval));
        }
        key
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SubscriptionManager {
    fn clone(&self) -> Self {
        Self {
            active_subscriptions: Arc::clone(&self.active_subscriptions),
            pending_subscriptions: Arc::clone(&self.pending_subscriptions),
        }
    }
}