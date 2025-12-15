//! Message parsing and data conversion

use crate::{
    data::*,
    error::{ParseError, ProcessingError},
    events::EventDispatcher,
};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

/// Trait for parsing WebSocket messages
pub trait DataParser: Send + Sync {
    fn parse_ticker(&self, data: &str) -> Result<TickerData, ParseError>;
    fn parse_orderbook(&self, data: &str) -> Result<OrderBookUpdate, ParseError>;
    fn parse_trade(&self, data: &str) -> Result<TradeData, ParseError>;
    fn parse_ohlc(&self, data: &str) -> Result<OHLCData, ParseError>;
}

/// Kraken-specific data parser
pub struct KrakenDataParser {
    // Internal parsing state and configuration
}

impl KrakenDataParser {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Parse JSON value safely
    fn parse_json(&self, data: &str) -> Result<Value, ParseError> {
        self.parse_json_robust(data)
    }
    
    /// Extract string field from JSON object
    fn extract_string(&self, obj: &Value, field: &str) -> Result<String, ParseError> {
        obj.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ParseError::MissingField(format!("Missing or invalid field: {}", field)))
    }
    
    /// Extract decimal field from JSON object
    fn extract_decimal(&self, obj: &Value, field: &str) -> Result<Decimal, ParseError> {
        let str_val = obj.get(field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::MissingField(format!("Missing field: {}", field)))?;
        
        Decimal::from_str(str_val)
            .map_err(|e| ParseError::InvalidDataType(format!("Invalid decimal for {}: {}", field, e)))
    }
    
    /// Extract timestamp from JSON object
    fn extract_timestamp(&self, obj: &Value, field: &str) -> Result<DateTime<Utc>, ParseError> {
        let timestamp_str = self.extract_string(obj, field)?;
        
        // Try parsing as Unix timestamp first
        if let Ok(timestamp_f64) = timestamp_str.parse::<f64>() {
            let timestamp_secs = timestamp_f64 as i64;
            let timestamp_nanos = ((timestamp_f64 - timestamp_secs as f64) * 1_000_000_000.0) as u32;
            
            DateTime::from_timestamp(timestamp_secs, timestamp_nanos)
                .ok_or_else(|| ParseError::InvalidDataType(format!("Invalid timestamp: {}", timestamp_str)))
        } else {
            // Try parsing as ISO 8601 string
            DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ParseError::InvalidDataType(format!("Invalid timestamp format: {}", e)))
        }
    }
    
    /// Parse trade side from string
    fn parse_trade_side(&self, side_str: &str) -> Result<TradeSide, ParseError> {
        match side_str.to_lowercase().as_str() {
            "b" | "buy" => Ok(TradeSide::Buy),
            "s" | "sell" => Ok(TradeSide::Sell),
            _ => Err(ParseError::InvalidDataType(format!("Invalid trade side: {}", side_str))),
        }
    }
    
    /// Handle malformed data gracefully
    fn handle_malformed_data(&self, error: ParseError, data: &str) -> ParseError {
        tracing::error!("Malformed data encountered: {} - Data: {}", error, data);
        
        // Log additional context for debugging
        tracing::debug!("Raw message length: {} bytes", data.len());
        
        // Try to extract any useful information from malformed data
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(event) = json.get("event") {
                tracing::debug!("Message event type: {:?}", event);
            }
            if let Some(channel) = json.get("channelName") {
                tracing::debug!("Message channel: {:?}", channel);
            }
        }
        
        error
    }
    
    /// Robust JSON parsing with error recovery
    fn parse_json_robust(&self, data: &str) -> Result<serde_json::Value, ParseError> {
        // First try normal parsing
        match serde_json::from_str(data) {
            Ok(json) => Ok(json),
            Err(e) => {
                // Try to clean up common JSON issues
                let cleaned_data = self.clean_json_data(data);
                serde_json::from_str(&cleaned_data)
                    .map_err(|_| ParseError::InvalidJson(format!("JSON parsing failed: {}", e)))
            }
        }
    }
    
    /// Clean up common JSON formatting issues
    fn clean_json_data(&self, data: &str) -> String {
        data.trim()
            .replace("\n", "")
            .replace("\r", "")
            .replace("\t", "")
    }
}

impl DataParser for KrakenDataParser {
    fn parse_ticker(&self, data: &str) -> Result<TickerData, ParseError> {
        let json = self.parse_json(data).map_err(|e| self.handle_malformed_data(e, data))?;
        
        // Kraken ticker format: [channelID, data, channelName, pair]
        if let Some(array) = json.as_array() {
            if array.len() >= 4 {
                let ticker_data = &array[1];
                let symbol = array[3].as_str()
                    .ok_or_else(|| ParseError::MissingField("symbol".to_string()))?
                    .to_string();
                
                if let Some(ticker_obj) = ticker_data.as_object() {
                    // Kraken ticker format has arrays for bid/ask/close/volume
                    // b = bid [price, wholeLotVolume, lotVolume]
                    // a = ask [price, wholeLotVolume, lotVolume]  
                    // c = close [price, lotVolume]
                    // v = volume [today, last24h]
                    
                    let bid = if let Some(bid_array) = ticker_obj.get("b").and_then(|v| v.as_array()) {
                        if let Some(price_str) = bid_array.get(0).and_then(|v| v.as_str()) {
                            Decimal::from_str(price_str).unwrap_or_default()
                        } else { Decimal::ZERO }
                    } else { Decimal::ZERO };
                    
                    let ask = if let Some(ask_array) = ticker_obj.get("a").and_then(|v| v.as_array()) {
                        if let Some(price_str) = ask_array.get(0).and_then(|v| v.as_str()) {
                            Decimal::from_str(price_str).unwrap_or_default()
                        } else { Decimal::ZERO }
                    } else { Decimal::ZERO };
                    
                    let last_price = if let Some(close_array) = ticker_obj.get("c").and_then(|v| v.as_array()) {
                        if let Some(price_str) = close_array.get(0).and_then(|v| v.as_str()) {
                            Decimal::from_str(price_str).unwrap_or_default()
                        } else { Decimal::ZERO }
                    } else { Decimal::ZERO };
                    
                    let volume = if let Some(vol_array) = ticker_obj.get("v").and_then(|v| v.as_array()) {
                        if let Some(vol_str) = vol_array.get(0).and_then(|v| v.as_str()) {
                            Decimal::from_str(vol_str).unwrap_or_default()
                        } else { Decimal::ZERO }
                    } else { Decimal::ZERO };
                    
                    // Convert Kraken symbol format to display format
                    let display_symbol = match symbol.as_str() {
                        "XBT/USD" => "BTC/USD".to_string(),
                        "ETH/USD" => "ETH/USD".to_string(),
                        "ADA/USD" => "ADA/USD".to_string(),
                        _ => symbol.clone(),
                    };
                    
                    return Ok(TickerData {
                        symbol: display_symbol,
                        bid,
                        ask,
                        last_price,
                        volume,
                        timestamp: Utc::now(),
                    });
                }
            }
        }
        
        Err(self.handle_malformed_data(
            ParseError::MalformedMessage("Invalid ticker message format".to_string()),
            data
        ))
    }
    
    fn parse_orderbook(&self, data: &str) -> Result<OrderBookUpdate, ParseError> {
        let json = self.parse_json(data).map_err(|e| self.handle_malformed_data(e, data))?;
        
        // Kraken orderbook format: [channelID, data, channelName, pair]
        if let Some(array) = json.as_array() {
            if array.len() >= 4 {
                let orderbook_data = &array[1];
                let symbol = array[3].as_str()
                    .ok_or_else(|| ParseError::MissingField("symbol".to_string()))?
                    .to_string();
                
                if let Some(ob_obj) = orderbook_data.as_object() {
                    let mut bids = Vec::new();
                    let mut asks = Vec::new();
                    
                    // Parse bids
                    if let Some(bids_array) = ob_obj.get("b").and_then(|v| v.as_array()) {
                        for bid in bids_array {
                            if let Some(bid_array) = bid.as_array() {
                                if bid_array.len() >= 3 {
                                    let price = Decimal::from_str(bid_array[0].as_str().unwrap_or("0"))
                                        .unwrap_or_default();
                                    let volume = Decimal::from_str(bid_array[1].as_str().unwrap_or("0"))
                                        .unwrap_or_default();
                                    let timestamp = DateTime::from_timestamp(
                                        bid_array[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0) as i64,
                                        0
                                    ).unwrap_or_else(Utc::now);
                                    
                                    bids.push(PriceLevel { price, volume, timestamp });
                                }
                            }
                        }
                    }
                    
                    // Parse asks
                    if let Some(asks_array) = ob_obj.get("a").and_then(|v| v.as_array()) {
                        for ask in asks_array {
                            if let Some(ask_array) = ask.as_array() {
                                if ask_array.len() >= 3 {
                                    let price = Decimal::from_str(ask_array[0].as_str().unwrap_or("0"))
                                        .unwrap_or_default();
                                    let volume = Decimal::from_str(ask_array[1].as_str().unwrap_or("0"))
                                        .unwrap_or_default();
                                    let timestamp = DateTime::from_timestamp(
                                        ask_array[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0) as i64,
                                        0
                                    ).unwrap_or_else(Utc::now);
                                    
                                    asks.push(PriceLevel { price, volume, timestamp });
                                }
                            }
                        }
                    }
                    
                    return Ok(OrderBookUpdate {
                        symbol,
                        bids,
                        asks,
                        timestamp: Utc::now(),
                        checksum: None, // Kraken provides checksums in some cases
                    });
                }
            }
        }
        
        Err(self.handle_malformed_data(
            ParseError::MalformedMessage("Invalid orderbook message format".to_string()),
            data
        ))
    }
    
    fn parse_trade(&self, data: &str) -> Result<TradeData, ParseError> {
        let json = self.parse_json(data).map_err(|e| self.handle_malformed_data(e, data))?;
        
        // Kraken trade format: [channelID, [[price, volume, time, side, orderType, misc]], channelName, pair]
        if let Some(array) = json.as_array() {
            if array.len() >= 4 {
                let trades_data = &array[1];
                let symbol = array[3].as_str()
                    .ok_or_else(|| ParseError::MissingField("symbol".to_string()))?
                    .to_string();
                
                if let Some(trades_array) = trades_data.as_array() {
                    if let Some(trade_array) = trades_array.first().and_then(|t| t.as_array()) {
                        if trade_array.len() >= 4 {
                            let price = Decimal::from_str(trade_array[0].as_str().unwrap_or("0"))
                                .unwrap_or_default();
                            let volume = Decimal::from_str(trade_array[1].as_str().unwrap_or("0"))
                                .unwrap_or_default();
                            let timestamp = DateTime::from_timestamp(
                                trade_array[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0) as i64,
                                0
                            ).unwrap_or_else(Utc::now);
                            let side = self.parse_trade_side(trade_array[3].as_str().unwrap_or("b"))?;
                            let trade_id = uuid::Uuid::new_v4().to_string();
                            
                            // Convert Kraken symbol format to display format
                            let display_symbol = match symbol.as_str() {
                                "XBT/USD" => "BTC/USD".to_string(),
                                "ETH/USD" => "ETH/USD".to_string(),
                                "ADA/USD" => "ADA/USD".to_string(),
                                _ => symbol.clone(),
                            };
                            
                            return Ok(TradeData {
                                symbol: display_symbol,
                                price,
                                volume,
                                side,
                                timestamp,
                                trade_id,
                            });
                        }
                    }
                }
            }
        }
        
        Err(self.handle_malformed_data(
            ParseError::MalformedMessage("Invalid trade message format".to_string()),
            data
        ))
    }
    
    fn parse_ohlc(&self, data: &str) -> Result<OHLCData, ParseError> {
        let json = self.parse_json(data).map_err(|e| self.handle_malformed_data(e, data))?;
        
        // Kraken OHLC format: [channelID, data, channelName, pair]
        if let Some(array) = json.as_array() {
            if array.len() >= 4 {
                let ohlc_data = &array[1];
                let symbol = array[3].as_str()
                    .ok_or_else(|| ParseError::MissingField("symbol".to_string()))?
                    .to_string();
                
                if let Some(ohlc_array) = ohlc_data.as_array() {
                    if ohlc_array.len() >= 8 {
                        let timestamp = DateTime::from_timestamp(
                            ohlc_array[0].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0) as i64,
                            0
                        ).unwrap_or_else(Utc::now);
                        let open = Decimal::from_str(ohlc_array[1].as_str().unwrap_or("0"))
                            .unwrap_or_default();
                        let high = Decimal::from_str(ohlc_array[2].as_str().unwrap_or("0"))
                            .unwrap_or_default();
                        let low = Decimal::from_str(ohlc_array[3].as_str().unwrap_or("0"))
                            .unwrap_or_default();
                        let close = Decimal::from_str(ohlc_array[4].as_str().unwrap_or("0"))
                            .unwrap_or_default();
                        let volume = Decimal::from_str(ohlc_array[6].as_str().unwrap_or("0"))
                            .unwrap_or_default();
                        
                        return Ok(OHLCData {
                            symbol,
                            open,
                            high,
                            low,
                            close,
                            volume,
                            timestamp,
                            interval: "1m".to_string(), // Default interval
                        });
                    }
                }
            }
        }
        
        Err(self.handle_malformed_data(
            ParseError::MalformedMessage("Invalid OHLC message format".to_string()),
            data
        ))
    }
}

impl Default for KrakenDataParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Message handler for routing WebSocket messages
#[derive(Clone)]
pub struct MessageHandler {
    parser: Arc<dyn DataParser>,
    dispatcher: Arc<EventDispatcher>,
}

impl MessageHandler {
    pub fn new(parser: Arc<dyn DataParser>, dispatcher: Arc<EventDispatcher>) -> Self {
        Self { parser, dispatcher }
    }
    
    /// Handle incoming WebSocket message
    pub async fn handle_message(&self, message: &str) -> Result<(), ProcessingError> {
        // Validate message format
        if message.is_empty() {
            return Err(ProcessingError::ProcessingFailed("Empty message received".to_string()));
        }
        
        // Log message for debugging
        tracing::debug!("Processing message: {}", message);
        
        // Try to determine message type and route accordingly
        if let Err(e) = self.route_message(message).await {
            tracing::warn!("Failed to route message: {} - Message: {}", e, message);
            return Err(e);
        }
        
        Ok(())
    }
    
    /// Route message to appropriate parser based on content
    async fn route_message(&self, message: &str) -> Result<(), ProcessingError> {
        // Check for subscription status messages first
        if message.contains("subscriptionStatus") {
            tracing::debug!("Received subscription status message");
            return Ok(());
        }
        
        // Check for system messages
        if message.contains("systemStatus") {
            tracing::debug!("Received system status message");
            return Ok(());
        }
        
        // Check for heartbeat messages
        if message.contains("heartbeat") {
            tracing::debug!("Received heartbeat message");
            return Ok(());
        }
        
        // Try to parse as market data
        if let Err(e) = self.try_parse_market_data(message).await {
            // If parsing fails, log but don't fail completely (graceful degradation)
            tracing::debug!("Could not parse as market data: {} - Message: {}", e, message);
        }
        
        Ok(())
    }
    
    /// Try to parse message as different market data types
    async fn try_parse_market_data(&self, message: &str) -> Result<(), ProcessingError> {
        // Try ticker data
        if let Ok(ticker_data) = self.parser.parse_ticker(message) {
            tracing::debug!("Parsed ticker data: {}", ticker_data.symbol);
            self.dispatcher.dispatch_ticker(ticker_data);
            return Ok(());
        }
        
        // Try order book data
        if let Ok(orderbook_data) = self.parser.parse_orderbook(message) {
            tracing::debug!("Parsed orderbook data: {}", orderbook_data.symbol);
            self.dispatcher.dispatch_orderbook(orderbook_data);
            return Ok(());
        }
        
        // Try trade data
        if let Ok(trade_data) = self.parser.parse_trade(message) {
            tracing::debug!("Parsed trade data: {}", trade_data.symbol);
            self.dispatcher.dispatch_trade(trade_data);
            return Ok(());
        }
        
        // Try OHLC data
        if let Ok(ohlc_data) = self.parser.parse_ohlc(message) {
            tracing::debug!("Parsed OHLC data: {}", ohlc_data.symbol);
            self.dispatcher.dispatch_ohlc(ohlc_data);
            return Ok(());
        }
        
        // If none of the parsers worked, return an error
        Err(ProcessingError::ProcessingFailed("Unknown message format".to_string()))
    }
    
    /// Register a new parser
    pub fn register_parser(&mut self, parser: Arc<dyn DataParser>) {
        self.parser = parser;
    }
    
    /// Validate message integrity
    fn validate_message(&self, message: &str) -> Result<(), ProcessingError> {
        // Basic JSON validation
        if let Err(e) = serde_json::from_str::<serde_json::Value>(message) {
            return Err(ProcessingError::ProcessingFailed(
                format!("Invalid JSON format: {}", e)
            ));
        }
        
        // Check for minimum message length
        if message.len() < 10 {
            return Err(ProcessingError::ProcessingFailed(
                "Message too short to be valid".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Handle malformed messages gracefully
    fn handle_malformed_message(&self, message: &str, error: &ProcessingError) {
        tracing::warn!("Malformed message encountered: {} - Message: {}", error, message);
        
        // Continue processing other messages (graceful degradation)
        // In a production system, you might want to:
        // 1. Increment error metrics
        // 2. Store malformed messages for analysis
        // 3. Alert monitoring systems if error rate is high
    }
}