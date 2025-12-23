//! Alert system for trading notifications
//!
//! Send alerts via various channels (webhook, console, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    /// Price crossed a threshold
    PriceAlert { symbol: String, price: f64, threshold: f64, direction: String },
    /// Order was filled
    OrderFilled { txid: String, symbol: String, side: String, volume: f64, price: f64 },
    /// Order was cancelled
    OrderCancelled { txid: String, reason: String },
    /// Position P&L threshold
    PnlAlert { symbol: String, pnl: f64, pnl_percent: f64 },
    /// Risk limit triggered
    RiskAlert { message: String, current_value: f64, limit: f64 },
    /// Strategy event
    StrategyEvent { strategy_name: String, event: String },
    /// Connection status
    ConnectionStatus { connected: bool, message: String },
    /// Custom alert
    Custom { title: String, message: String },
}

/// An alert to be sent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
}

impl Alert {
    pub fn new(alert_type: AlertType, severity: AlertSeverity) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            alert_type,
            severity,
            timestamp: Utc::now(),
            acknowledged: false,
        }
    }

    pub fn info(alert_type: AlertType) -> Self {
        Self::new(alert_type, AlertSeverity::Info)
    }

    pub fn warning(alert_type: AlertType) -> Self {
        Self::new(alert_type, AlertSeverity::Warning)
    }

    pub fn critical(alert_type: AlertType) -> Self {
        Self::new(alert_type, AlertSeverity::Critical)
    }

    /// Format alert as a message string
    pub fn format_message(&self) -> String {
        let emoji = match self.severity {
            AlertSeverity::Info => "ℹ️",
            AlertSeverity::Warning => "⚠️",
            AlertSeverity::Critical => "🚨",
        };

        let body = match &self.alert_type {
            AlertType::PriceAlert { symbol, price, threshold, direction } => {
                format!("{} {} {} ${:.2} (threshold: ${:.2})", symbol, direction, price, price, threshold)
            }
            AlertType::OrderFilled { txid, symbol, side, volume, price } => {
                format!("Order filled: {} {} {} @ ${:.2} [{}]", side.to_uppercase(), volume, symbol, price, &txid[..8])
            }
            AlertType::OrderCancelled { txid, reason } => {
                format!("Order cancelled: {} - {}", &txid[..8], reason)
            }
            AlertType::PnlAlert { symbol, pnl, pnl_percent } => {
                let sign = if *pnl >= 0.0 { "+" } else { "" };
                format!("{} P&L: {}${:.2} ({}{:.2}%)", symbol, sign, pnl, sign, pnl_percent)
            }
            AlertType::RiskAlert { message, current_value, limit } => {
                format!("RISK: {} (current: {:.2}, limit: {:.2})", message, current_value, limit)
            }
            AlertType::StrategyEvent { strategy_name, event } => {
                format!("[{}] {}", strategy_name, event)
            }
            AlertType::ConnectionStatus { connected, message } => {
                let status = if *connected { "Connected" } else { "Disconnected" };
                format!("{}: {}", status, message)
            }
            AlertType::Custom { title, message } => {
                format!("{}: {}", title, message)
            }
        };

        format!("{} {}", emoji, body)
    }
}

/// Alert channel trait
#[async_trait::async_trait]
pub trait AlertChannel: Send + Sync {
    async fn send(&self, alert: &Alert) -> Result<(), String>;
    fn name(&self) -> &str;
}

/// Console alert channel (logs to stdout)
pub struct ConsoleChannel;

#[async_trait::async_trait]
impl AlertChannel for ConsoleChannel {
    async fn send(&self, alert: &Alert) -> Result<(), String> {
        println!("[ALERT] {}", alert.format_message());
        Ok(())
    }

    fn name(&self) -> &str {
        "console"
    }
}

/// Webhook alert channel (HTTP POST)
pub struct WebhookChannel {
    pub url: String,
    pub headers: HashMap<String, String>,
}

impl WebhookChannel {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            headers: HashMap::new(),
        }
    }

    pub fn discord(webhook_url: &str) -> Self {
        Self::new(webhook_url)
    }

    pub fn telegram(bot_token: &str, chat_id: &str) -> Self {
        Self::new(&format!(
            "https://api.telegram.org/bot{}/sendMessage?chat_id={}",
            bot_token, chat_id
        ))
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }
}

#[async_trait::async_trait]
impl AlertChannel for WebhookChannel {
    async fn send(&self, alert: &Alert) -> Result<(), String> {
        let client = reqwest::Client::new();
        
        // Format for Discord/Slack style webhooks
        let payload = serde_json::json!({
            "content": alert.format_message(),
            "text": alert.format_message(),  // Slack format
            "embeds": [{
                "title": format!("{:?} Alert", alert.severity),
                "description": alert.format_message(),
                "color": match alert.severity {
                    AlertSeverity::Info => 3447003,      // Blue
                    AlertSeverity::Warning => 16776960,  // Yellow
                    AlertSeverity::Critical => 15158332, // Red
                },
                "timestamp": alert.timestamp.to_rfc3339(),
            }]
        });

        let mut request = client.post(&self.url).json(&payload);
        
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("Webhook returned {}", response.status()))
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn name(&self) -> &str {
        "webhook"
    }
}

/// Alert manager - routes alerts to channels
pub struct AlertManager {
    channels: Vec<Box<dyn AlertChannel>>,
    history: Vec<Alert>,
    max_history: usize,
    /// Filter by minimum severity
    min_severity: AlertSeverity,
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            channels: vec![Box::new(ConsoleChannel)],
            history: Vec::new(),
            max_history: 1000,
            min_severity: AlertSeverity::Info,
        }
    }

    pub fn add_channel(&mut self, channel: Box<dyn AlertChannel>) {
        self.channels.push(channel);
    }

    pub fn set_min_severity(&mut self, severity: AlertSeverity) {
        self.min_severity = severity;
    }

    /// Send an alert to all channels
    pub async fn send(&mut self, alert: Alert) {
        // Check severity filter
        let dominated = match (&alert.severity, &self.min_severity) {
            (AlertSeverity::Info, AlertSeverity::Warning) => true,
            (AlertSeverity::Info, AlertSeverity::Critical) => true,
            (AlertSeverity::Warning, AlertSeverity::Critical) => true,
            _ => false,
        };

        if dominated {
            return;
        }

        // Send to all channels
        for channel in &self.channels {
            if let Err(e) = channel.send(&alert).await {
                eprintln!("Failed to send alert via {}: {}", channel.name(), e);
            }
        }

        // Store in history
        self.history.push(alert);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Get alert history
    pub fn get_history(&self, count: usize) -> Vec<&Alert> {
        self.history.iter().rev().take(count).collect()
    }

    /// Get unacknowledged alerts
    pub fn get_unacknowledged(&self) -> Vec<&Alert> {
        self.history.iter().filter(|a| !a.acknowledged).collect()
    }

    /// Acknowledge an alert
    pub fn acknowledge(&mut self, alert_id: &str) {
        if let Some(alert) = self.history.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
        }
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for creating alerts
pub fn price_alert(symbol: &str, price: f64, threshold: f64, above: bool) -> Alert {
    Alert::info(AlertType::PriceAlert {
        symbol: symbol.to_string(),
        price,
        threshold,
        direction: if above { "above".to_string() } else { "below".to_string() },
    })
}

pub fn order_filled(txid: &str, symbol: &str, side: &str, volume: f64, price: f64) -> Alert {
    Alert::info(AlertType::OrderFilled {
        txid: txid.to_string(),
        symbol: symbol.to_string(),
        side: side.to_string(),
        volume,
        price,
    })
}

pub fn pnl_alert(symbol: &str, pnl: f64, pnl_percent: f64, is_warning: bool) -> Alert {
    let alert_type = AlertType::PnlAlert {
        symbol: symbol.to_string(),
        pnl,
        pnl_percent,
    };
    if is_warning {
        Alert::warning(alert_type)
    } else {
        Alert::info(alert_type)
    }
}

pub fn risk_alert(message: &str, current: f64, limit: f64) -> Alert {
    Alert::critical(AlertType::RiskAlert {
        message: message.to_string(),
        current_value: current,
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_formatting() {
        let alert = price_alert("XBT/USD", 50000.0, 49000.0, true);
        let msg = alert.format_message();
        assert!(msg.contains("XBT/USD"));
        assert!(msg.contains("above"));
    }

    #[test]
    fn test_order_filled_alert() {
        let alert = order_filled("ABC123", "XBT/USD", "buy", 0.1, 50000.0);
        let msg = alert.format_message();
        assert!(msg.contains("BUY"));
        assert!(msg.contains("0.1"));
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let mut manager = AlertManager::new();
        let alert = Alert::info(AlertType::Custom {
            title: "Test".to_string(),
            message: "Hello".to_string(),
        });
        manager.send(alert).await;
        assert_eq!(manager.history.len(), 1);
    }
}
