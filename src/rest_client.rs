//! REST API client for Kraken
//!
//! Provides authenticated access to Kraken's REST API for:
//! - Account data (balances, positions, trade history)
//! - Order management (place, cancel, edit orders)
//! - Market data queries

use crate::auth::Credentials;
use crate::error::SdkError;
use crate::rate_limit::{AccountTier, EndpointCost, RateLimiter};
use crate::trading::*;
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

const KRAKEN_API_URL: &str = "https://api.kraken.com";
const API_VERSION: &str = "0";

/// REST API client for Kraken
pub struct KrakenRestClient {
    credentials: Credentials,
    rate_limiter: Arc<RateLimiter>,
    http_client: reqwest::Client,
}

impl KrakenRestClient {
    /// Create a new REST client with credentials
    pub fn new(credentials: Credentials) -> Self {
        Self::with_tier(credentials, AccountTier::default())
    }

    /// Create a new REST client with specific account tier
    pub fn with_tier(credentials: Credentials, tier: AccountTier) -> Self {
        Self {
            credentials,
            rate_limiter: Arc::new(RateLimiter::new(tier)),
            http_client: reqwest::Client::new(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, SdkError> {
        let credentials = Credentials::from_env()?;
        Ok(Self::new(credentials))
    }

    /// Get the rate limiter for monitoring
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    // ========== Account Endpoints ==========

    /// Get account balances
    pub async fn get_balance(&self) -> Result<Balances, SdkError> {
        let response: HashMap<String, String> = self.private_request("Balance", &[], EndpointCost::Standard).await?;
        
        let mut balances = Balances::default();
        for (asset, balance_str) in response {
            let balance: Decimal = balance_str.parse()
                .map_err(|e| SdkError::Parse(crate::error::ParseError::InvalidDataType(
                    format!("Invalid balance: {}", e)
                )))?;
            
            balances.assets.insert(asset.clone(), AssetBalance {
                asset,
                balance,
                available: balance, // Full balance available (no hold info from this endpoint)
                hold: Decimal::ZERO,
            });
        }
        
        Ok(balances)
    }

    /// Get extended balance info (includes holds)
    pub async fn get_balance_extended(&self) -> Result<Balances, SdkError> {
        let response: HashMap<String, Value> = self.private_request("BalanceEx", &[], EndpointCost::Standard).await?;
        
        let mut balances = Balances::default();
        for (asset, info) in response {
            let balance: Decimal = info["balance"].as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(Decimal::ZERO);
            let hold: Decimal = info["hold_trade"].as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(Decimal::ZERO);
            
            balances.assets.insert(asset.clone(), AssetBalance {
                asset,
                balance,
                available: balance - hold,
                hold,
            });
        }
        
        Ok(balances)
    }

    /// Get trade history
    pub async fn get_trades_history(&self, opts: TradesHistoryOptions) -> Result<Vec<Execution>, SdkError> {
        let mut params = Vec::new();
        
        if let Some(start) = opts.start {
            params.push(("start".to_string(), start.timestamp().to_string()));
        }
        if let Some(end) = opts.end {
            params.push(("end".to_string(), end.timestamp().to_string()));
        }
        if let Some(ofs) = opts.offset {
            params.push(("ofs".to_string(), ofs.to_string()));
        }

        let response: Value = self.private_request("TradesHistory", &params, EndpointCost::Ledger).await?;
        
        let trades = response["trades"].as_object()
            .ok_or_else(|| SdkError::Parse(crate::error::ParseError::MissingField("trades".to_string())))?;
        
        let mut executions = Vec::new();
        for (trade_id, trade_data) in trades {
            if let Ok(exec) = parse_execution(trade_id, trade_data) {
                executions.push(exec);
            }
        }
        
        // Sort by time descending
        executions.sort_by(|a, b| b.time.cmp(&a.time));
        
        Ok(executions)
    }

    /// Get open orders
    pub async fn get_open_orders(&self) -> Result<Vec<Order>, SdkError> {
        let response: Value = self.private_request("OpenOrders", &[], EndpointCost::Standard).await?;
        
        let orders = response["open"].as_object()
            .ok_or_else(|| SdkError::Parse(crate::error::ParseError::MissingField("open".to_string())))?;
        
        let mut result = Vec::new();
        for (txid, order_data) in orders {
            if let Ok(order) = parse_order(txid, order_data) {
                result.push(order);
            }
        }
        
        Ok(result)
    }

    /// Get closed orders
    pub async fn get_closed_orders(&self, opts: ClosedOrdersOptions) -> Result<Vec<Order>, SdkError> {
        let mut params = Vec::new();
        
        if let Some(start) = opts.start {
            params.push(("start".to_string(), start.timestamp().to_string()));
        }
        if let Some(end) = opts.end {
            params.push(("end".to_string(), end.timestamp().to_string()));
        }
        if let Some(ofs) = opts.offset {
            params.push(("ofs".to_string(), ofs.to_string()));
        }

        let response: Value = self.private_request("ClosedOrders", &params, EndpointCost::Ledger).await?;
        
        let orders = response["closed"].as_object()
            .ok_or_else(|| SdkError::Parse(crate::error::ParseError::MissingField("closed".to_string())))?;
        
        let mut result = Vec::new();
        for (txid, order_data) in orders {
            if let Ok(order) = parse_order(txid, order_data) {
                result.push(order);
            }
        }
        
        Ok(result)
    }

    /// Get open positions
    pub async fn get_open_positions(&self) -> Result<Vec<Position>, SdkError> {
        let response: Value = self.private_request("OpenPositions", &[], EndpointCost::Standard).await?;
        
        let positions = response.as_object()
            .ok_or_else(|| SdkError::Parse(crate::error::ParseError::InvalidDataType("Expected object".to_string())))?;
        
        let mut result = Vec::new();
        for (pos_id, pos_data) in positions {
            if let Ok(position) = parse_position(pos_id, pos_data) {
                result.push(position);
            }
        }
        
        Ok(result)
    }

    // ========== Trading Endpoints ==========

    /// Place a new order
    pub async fn add_order(&self, request: OrderRequest) -> Result<OrderResponse, SdkError> {
        let params = request.to_params();
        let params_ref: Vec<(String, String)> = params.into_iter().collect();
        
        self.private_request("AddOrder", &params_ref, EndpointCost::Order).await
    }

    /// Place multiple orders (batch)
    pub async fn add_order_batch(&self, requests: Vec<OrderRequest>) -> Result<Vec<Result<OrderResponse, SdkError>>, SdkError> {
        // Kraken doesn't have native batch - execute sequentially with rate limiting
        let mut results = Vec::new();
        
        for request in requests {
            let result = self.add_order(request).await;
            results.push(result);
        }
        
        Ok(results)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, txid: &str) -> Result<CancelResponse, SdkError> {
        let params = vec![("txid".to_string(), txid.to_string())];
        self.private_request("CancelOrder", &params, EndpointCost::Order).await
    }

    /// Cancel all open orders
    pub async fn cancel_all(&self) -> Result<CancelResponse, SdkError> {
        self.private_request("CancelAll", &[], EndpointCost::Order).await
    }

    /// Cancel all orders for a specific pair
    pub async fn cancel_all_orders_after(&self, timeout_seconds: u32) -> Result<Value, SdkError> {
        let params = vec![("timeout".to_string(), timeout_seconds.to_string())];
        self.private_request("CancelAllOrdersAfter", &params, EndpointCost::Order).await
    }

    /// Edit an existing order
    pub async fn edit_order(&self, request: EditOrderRequest) -> Result<OrderResponse, SdkError> {
        let mut params = vec![("txid".to_string(), request.txid)];
        
        if let Some(volume) = request.volume {
            params.push(("volume".to_string(), volume.to_string()));
        }
        if let Some(price) = request.price {
            params.push(("price".to_string(), price.to_string()));
        }
        if let Some(price2) = request.price2 {
            params.push(("price2".to_string(), price2.to_string()));
        }
        
        self.private_request("EditOrder", &params, EndpointCost::Order).await
    }

    // ========== WebSocket Token ==========

    /// Get WebSocket authentication token
    pub async fn get_websocket_token(&self) -> Result<String, SdkError> {
        let response: Value = self.private_request("GetWebSocketsToken", &[], EndpointCost::Standard).await?;
        
        response["token"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SdkError::Parse(crate::error::ParseError::MissingField("token".to_string())))
    }

    // ========== Internal Methods ==========

    /// Make an authenticated private API request
    async fn private_request<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(String, String)],
        cost: EndpointCost,
    ) -> Result<T, SdkError> {
        // Acquire rate limit
        self.rate_limiter.acquire(cost).await?;

        let path = format!("/{}/private/{}", API_VERSION, endpoint);
        let url = format!("{}{}", KRAKEN_API_URL, path);
        
        // Generate nonce and build post data
        let nonce = Credentials::generate_nonce();
        let mut post_params = vec![("nonce".to_string(), nonce.to_string())];
        post_params.extend(params.iter().cloned());
        
        let post_data: String = post_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        
        // Sign the request
        let signature = self.credentials.sign(&path, nonce, &post_data)?;
        
        // Make the request
        let response = self.http_client
            .post(&url)
            .header("API-Key", self.credentials.api_key())
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data)
            .send()
            .await
            .map_err(|e| SdkError::Network(e.to_string()))?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| SdkError::Network(e.to_string()))?;

        tracing::debug!("Kraken API response [{}]: {}", endpoint, &body[..body.len().min(200)]);

        // Parse response
        let json: Value = serde_json::from_str(&body)
            .map_err(|e| SdkError::Parse(crate::error::ParseError::InvalidJson(e.to_string())))?;

        // Check for errors
        if let Some(errors) = json["error"].as_array() {
            if !errors.is_empty() {
                let error_msgs: Vec<String> = errors
                    .iter()
                    .filter_map(|e| e.as_str().map(|s| s.to_string()))
                    .collect();
                return Err(SdkError::Network(error_msgs.join(", ")));
            }
        }

        if !status.is_success() {
            return Err(SdkError::Network(format!("HTTP {}: {}", status, body)));
        }

        // Extract result
        let result = json.get("result")
            .ok_or_else(|| SdkError::Parse(crate::error::ParseError::MissingField("result".to_string())))?;

        serde_json::from_value(result.clone())
            .map_err(|e| SdkError::Parse(crate::error::ParseError::InvalidDataType(e.to_string())))
    }
}

// ========== Options Structs ==========

/// Options for trades history query
#[derive(Debug, Clone, Default)]
pub struct TradesHistoryOptions {
    pub start: Option<chrono::DateTime<chrono::Utc>>,
    pub end: Option<chrono::DateTime<chrono::Utc>>,
    pub offset: Option<u32>,
}

/// Options for closed orders query
#[derive(Debug, Clone, Default)]
pub struct ClosedOrdersOptions {
    pub start: Option<chrono::DateTime<chrono::Utc>>,
    pub end: Option<chrono::DateTime<chrono::Utc>>,
    pub offset: Option<u32>,
}

// ========== Parsing Helpers ==========

fn parse_execution(trade_id: &str, data: &Value) -> Result<Execution, SdkError> {
    Ok(Execution {
        trade_id: trade_id.to_string(),
        order_txid: data["ordertxid"].as_str().unwrap_or("").to_string(),
        pair: data["pair"].as_str().unwrap_or("").to_string(),
        side: if data["type"].as_str() == Some("buy") { OrderSide::Buy } else { OrderSide::Sell },
        order_type: parse_order_type(data["ordertype"].as_str().unwrap_or("market")),
        price: parse_decimal(data["price"].as_str()),
        volume: parse_decimal(data["vol"].as_str()),
        cost: parse_decimal(data["cost"].as_str()),
        fee: parse_decimal(data["fee"].as_str()),
        fee_currency: data["fee"].as_str().unwrap_or("USD").to_string(),
        time: parse_timestamp(data["time"].as_f64()),
    })
}

fn parse_order(txid: &str, data: &Value) -> Result<Order, SdkError> {
    let descr = &data["descr"];
    
    Ok(Order {
        txid: txid.to_string(),
        status: parse_order_status(data["status"].as_str().unwrap_or("open")),
        pair: descr["pair"].as_str().unwrap_or("").to_string(),
        side: if descr["type"].as_str() == Some("buy") { OrderSide::Buy } else { OrderSide::Sell },
        order_type: parse_order_type(descr["ordertype"].as_str().unwrap_or("limit")),
        volume: parse_decimal(data["vol"].as_str()),
        volume_exec: parse_decimal(data["vol_exec"].as_str()),
        price: data["descr"]["price"].as_str().and_then(|s| s.parse().ok()),
        avg_price: data["price"].as_str().and_then(|s| s.parse().ok()),
        opentm: parse_timestamp(data["opentm"].as_f64()),
        closetm: data["closetm"].as_f64().map(|t| parse_timestamp(Some(t))),
        client_order_id: data["cl_ord_id"].as_str().map(|s| s.to_string()),
    })
}

fn parse_position(pos_id: &str, data: &Value) -> Result<Position, SdkError> {
    Ok(Position {
        position_id: pos_id.to_string(),
        pair: data["pair"].as_str().unwrap_or("").to_string(),
        side: if data["type"].as_str() == Some("buy") { OrderSide::Buy } else { OrderSide::Sell },
        volume: parse_decimal(data["vol"].as_str()),
        entry_price: parse_decimal(data["cost"].as_str()) / parse_decimal(data["vol"].as_str()).max(Decimal::ONE),
        mark_price: parse_decimal(data["value"].as_str()) / parse_decimal(data["vol"].as_str()).max(Decimal::ONE),
        unrealized_pnl: parse_decimal(data["net"].as_str()),
        realized_pnl: Decimal::ZERO,
        liquidation_price: None,
        open_time: parse_timestamp(data["opentm"].as_f64()),
    })
}

fn parse_decimal(s: Option<&str>) -> Decimal {
    s.and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO)
}

fn parse_timestamp(ts: Option<f64>) -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    ts.map(|t| chrono::Utc.timestamp_opt(t as i64, ((t.fract()) * 1_000_000_000.0) as u32).unwrap())
        .unwrap_or_else(chrono::Utc::now)
}

fn parse_order_type(s: &str) -> OrderType {
    match s {
        "market" => OrderType::Market,
        "limit" => OrderType::Limit,
        "stop-loss" => OrderType::StopLoss,
        "take-profit" => OrderType::TakeProfit,
        "stop-loss-limit" => OrderType::StopLossLimit,
        "take-profit-limit" => OrderType::TakeProfitLimit,
        _ => OrderType::Limit,
    }
}

fn parse_order_status(s: &str) -> OrderStatus {
    match s {
        "pending" => OrderStatus::Pending,
        "open" => OrderStatus::Open,
        "closed" => OrderStatus::Closed,
        "canceled" => OrderStatus::Canceled,
        "expired" => OrderStatus::Expired,
        _ => OrderStatus::Open,
    }
}

impl std::fmt::Debug for KrakenRestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KrakenRestClient")
            .field("credentials", &self.credentials)
            .field("rate_limiter", &self.rate_limiter.stats())
            .finish()
    }
}
