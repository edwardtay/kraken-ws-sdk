//! Trading types and order management
//!
//! Provides order types, builders, and execution tracking for Kraken trading.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
    StopLossLimit,
    TakeProfitLimit,
    SettlePosition,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderType::Market => write!(f, "market"),
            OrderType::Limit => write!(f, "limit"),
            OrderType::StopLoss => write!(f, "stop-loss"),
            OrderType::TakeProfit => write!(f, "take-profit"),
            OrderType::StopLossLimit => write!(f, "stop-loss-limit"),
            OrderType::TakeProfitLimit => write!(f, "take-profit-limit"),
            OrderType::SettlePosition => write!(f, "settle-position"),
        }
    }
}

/// Time in force for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good till cancelled (default)
    GTC,
    /// Immediate or cancel
    IOC,
    /// Good till date
    GTD,
}

impl Default for TimeInForce {
    fn default() -> Self {
        TimeInForce::GTC
    }
}

/// Order flags
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderFlags {
    /// Post-only order (maker only)
    pub post_only: bool,
    /// Fee in quote currency
    pub fee_in_quote: bool,
    /// No market price protection
    pub no_mpp: bool,
    /// Reduce only (futures)
    pub reduce_only: bool,
}

/// Request to place a new order
#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    /// Trading pair (e.g., "XBT/USD")
    pub pair: String,
    /// Buy or sell
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Order volume in base currency
    pub volume: Decimal,
    /// Limit price (required for limit orders)
    pub price: Option<Decimal>,
    /// Secondary price (for stop-loss-limit, take-profit-limit)
    pub price2: Option<Decimal>,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Order flags
    pub flags: OrderFlags,
    /// Client order ID (optional, for tracking)
    pub client_order_id: Option<String>,
    /// Validate only (don't submit)
    pub validate: bool,
}

impl OrderRequest {
    /// Create a market buy order
    pub fn market_buy(pair: &str, volume: Decimal) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            volume,
            price: None,
            price2: None,
            time_in_force: TimeInForce::default(),
            flags: OrderFlags::default(),
            client_order_id: None,
            validate: false,
        }
    }

    /// Create a market sell order
    pub fn market_sell(pair: &str, volume: Decimal) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Sell,
            order_type: OrderType::Market,
            volume,
            price: None,
            price2: None,
            time_in_force: TimeInForce::default(),
            flags: OrderFlags::default(),
            client_order_id: None,
            validate: false,
        }
    }

    /// Create a limit buy order
    pub fn limit_buy(pair: &str, volume: Decimal, price: Decimal) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            volume,
            price: Some(price),
            price2: None,
            time_in_force: TimeInForce::default(),
            flags: OrderFlags::default(),
            client_order_id: None,
            validate: false,
        }
    }

    /// Create a limit sell order
    pub fn limit_sell(pair: &str, volume: Decimal, price: Decimal) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Sell,
            order_type: OrderType::Limit,
            volume,
            price: Some(price),
            price2: None,
            time_in_force: TimeInForce::default(),
            flags: OrderFlags::default(),
            client_order_id: None,
            validate: false,
        }
    }

    /// Set time in force
    pub fn with_time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = tif;
        self
    }

    /// Set as post-only (maker only)
    pub fn post_only(mut self) -> Self {
        self.flags.post_only = true;
        self
    }

    /// Set as reduce-only
    pub fn reduce_only(mut self) -> Self {
        self.flags.reduce_only = true;
        self
    }

    /// Set client order ID for tracking
    pub fn with_client_id(mut self, id: &str) -> Self {
        self.client_order_id = Some(id.to_string());
        self
    }

    /// Set to validate only (don't actually submit)
    pub fn validate_only(mut self) -> Self {
        self.validate = true;
        self
    }

    /// Convert to Kraken API parameters
    pub fn to_params(&self) -> Vec<(String, String)> {
        let mut params = vec![
            ("pair".to_string(), self.pair.clone()),
            ("type".to_string(), self.side.to_string()),
            ("ordertype".to_string(), self.order_type.to_string()),
            ("volume".to_string(), self.volume.to_string()),
        ];

        if let Some(price) = &self.price {
            params.push(("price".to_string(), price.to_string()));
        }

        if let Some(price2) = &self.price2 {
            params.push(("price2".to_string(), price2.to_string()));
        }

        match self.time_in_force {
            TimeInForce::IOC => params.push(("timeinforce".to_string(), "IOC".to_string())),
            TimeInForce::GTD => params.push(("timeinforce".to_string(), "GTD".to_string())),
            TimeInForce::GTC => {} // Default, don't send
        }

        let mut flags = Vec::new();
        if self.flags.post_only {
            flags.push("post");
        }
        if self.flags.fee_in_quote {
            flags.push("fciq");
        }
        if self.flags.no_mpp {
            flags.push("nompp");
        }
        if self.flags.reduce_only {
            flags.push("reduceonly");
        }
        if !flags.is_empty() {
            params.push(("oflags".to_string(), flags.join(",")));
        }

        if let Some(ref client_id) = self.client_order_id {
            params.push(("cl_ord_id".to_string(), client_id.clone()));
        }

        if self.validate {
            params.push(("validate".to_string(), "true".to_string()));
        }

        params
    }
}

/// Response from placing an order
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    /// Transaction IDs of the order(s)
    pub txid: Vec<String>,
    /// Order description
    pub descr: OrderDescription,
}

/// Order description from Kraken
#[derive(Debug, Clone, Deserialize)]
pub struct OrderDescription {
    /// Order description string
    pub order: String,
    /// Close order description (if applicable)
    pub close: Option<String>,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Open,
    Closed,
    Canceled,
    Expired,
}

/// Full order information
#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    /// Order transaction ID
    pub txid: String,
    /// Order status
    pub status: OrderStatus,
    /// Trading pair
    pub pair: String,
    /// Order side
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Original volume
    pub volume: Decimal,
    /// Executed volume
    pub volume_exec: Decimal,
    /// Limit price
    pub price: Option<Decimal>,
    /// Average execution price
    pub avg_price: Option<Decimal>,
    /// Order creation time
    pub opentm: DateTime<Utc>,
    /// Order close time (if closed)
    pub closetm: Option<DateTime<Utc>>,
    /// Client order ID
    pub client_order_id: Option<String>,
}

impl Order {
    /// Check if order is fully filled
    pub fn is_filled(&self) -> bool {
        self.volume_exec >= self.volume
    }

    /// Check if order is still active
    pub fn is_active(&self) -> bool {
        matches!(self.status, OrderStatus::Pending | OrderStatus::Open)
    }

    /// Get remaining volume
    pub fn remaining_volume(&self) -> Decimal {
        self.volume - self.volume_exec
    }

    /// Get fill percentage
    pub fn fill_percent(&self) -> Decimal {
        if self.volume.is_zero() {
            Decimal::ZERO
        } else {
            (self.volume_exec / self.volume) * Decimal::from(100)
        }
    }
}

/// Execution report (trade/fill)
#[derive(Debug, Clone, Deserialize)]
pub struct Execution {
    /// Trade ID
    pub trade_id: String,
    /// Order transaction ID
    pub order_txid: String,
    /// Trading pair
    pub pair: String,
    /// Trade side
    pub side: OrderSide,
    /// Trade type
    pub order_type: OrderType,
    /// Execution price
    pub price: Decimal,
    /// Execution volume
    pub volume: Decimal,
    /// Trade cost (price × volume)
    pub cost: Decimal,
    /// Fee amount
    pub fee: Decimal,
    /// Fee currency
    pub fee_currency: String,
    /// Execution time
    pub time: DateTime<Utc>,
}

/// Request to cancel an order
#[derive(Debug, Clone)]
pub struct CancelRequest {
    /// Transaction ID to cancel
    pub txid: String,
}

impl CancelRequest {
    pub fn new(txid: &str) -> Self {
        Self {
            txid: txid.to_string(),
        }
    }
}

/// Response from canceling an order
#[derive(Debug, Clone, Deserialize)]
pub struct CancelResponse {
    /// Number of orders canceled
    pub count: u32,
    /// Whether order was pending (not yet in book)
    pub pending: Option<bool>,
}

/// Request to edit an existing order
#[derive(Debug, Clone)]
pub struct EditOrderRequest {
    /// Transaction ID of order to edit
    pub txid: String,
    /// New volume (optional)
    pub volume: Option<Decimal>,
    /// New price (optional)
    pub price: Option<Decimal>,
    /// New secondary price (optional)
    pub price2: Option<Decimal>,
}

impl EditOrderRequest {
    pub fn new(txid: &str) -> Self {
        Self {
            txid: txid.to_string(),
            volume: None,
            price: None,
            price2: None,
        }
    }

    pub fn with_volume(mut self, volume: Decimal) -> Self {
        self.volume = Some(volume);
        self
    }

    pub fn with_price(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }
}

/// Account balance for a single asset
#[derive(Debug, Clone, Deserialize)]
pub struct AssetBalance {
    /// Asset name
    pub asset: String,
    /// Total balance
    pub balance: Decimal,
    /// Available balance (not in orders)
    pub available: Decimal,
    /// Balance held in orders
    pub hold: Decimal,
}

/// Full account balances
#[derive(Debug, Clone, Default)]
pub struct Balances {
    /// Map of asset -> balance
    pub assets: std::collections::HashMap<String, AssetBalance>,
}

impl Balances {
    /// Get balance for a specific asset
    pub fn get(&self, asset: &str) -> Option<&AssetBalance> {
        self.assets.get(asset)
    }

    /// Get available balance for an asset
    pub fn available(&self, asset: &str) -> Decimal {
        self.assets
            .get(asset)
            .map(|b| b.available)
            .unwrap_or(Decimal::ZERO)
    }

    /// Get total balance for an asset
    pub fn total(&self, asset: &str) -> Decimal {
        self.assets
            .get(asset)
            .map(|b| b.balance)
            .unwrap_or(Decimal::ZERO)
    }
}

/// Open position
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    /// Position ID
    pub position_id: String,
    /// Trading pair
    pub pair: String,
    /// Position side (long = buy, short = sell)
    pub side: OrderSide,
    /// Position size
    pub volume: Decimal,
    /// Average entry price
    pub entry_price: Decimal,
    /// Current mark price
    pub mark_price: Decimal,
    /// Unrealized P&L
    pub unrealized_pnl: Decimal,
    /// Realized P&L
    pub realized_pnl: Decimal,
    /// Liquidation price (if applicable)
    pub liquidation_price: Option<Decimal>,
    /// Position open time
    pub open_time: DateTime<Utc>,
}

impl Position {
    /// Calculate P&L percentage
    pub fn pnl_percent(&self) -> Decimal {
        if self.entry_price.is_zero() {
            return Decimal::ZERO;
        }
        
        let pnl = match self.side {
            OrderSide::Buy => self.mark_price - self.entry_price,
            OrderSide::Sell => self.entry_price - self.mark_price,
        };
        
        (pnl / self.entry_price) * Decimal::from(100)
    }

    /// Check if position is profitable
    pub fn is_profitable(&self) -> bool {
        self.unrealized_pnl > Decimal::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_market_order_creation() {
        let order = OrderRequest::market_buy("XBT/USD", dec!(0.001));
        assert_eq!(order.pair, "XBT/USD");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.order_type, OrderType::Market);
        assert!(order.price.is_none());
    }

    #[test]
    fn test_limit_order_creation() {
        let order = OrderRequest::limit_sell("ETH/USD", dec!(1.5), dec!(2000.00));
        assert_eq!(order.side, OrderSide::Sell);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.price, Some(dec!(2000.00)));
    }

    #[test]
    fn test_order_builder_chain() {
        let order = OrderRequest::limit_buy("XBT/USD", dec!(0.01), dec!(50000.00))
            .post_only()
            .with_client_id("my-order-123")
            .with_time_in_force(TimeInForce::IOC);

        assert!(order.flags.post_only);
        assert_eq!(order.client_order_id, Some("my-order-123".to_string()));
        assert_eq!(order.time_in_force, TimeInForce::IOC);
    }

    #[test]
    fn test_order_to_params() {
        let order = OrderRequest::limit_buy("XBT/USD", dec!(0.01), dec!(50000.00))
            .post_only();
        
        let params = order.to_params();
        assert!(params.iter().any(|(k, v)| k == "pair" && v == "XBT/USD"));
        assert!(params.iter().any(|(k, v)| k == "type" && v == "buy"));
        assert!(params.iter().any(|(k, v)| k == "ordertype" && v == "limit"));
        assert!(params.iter().any(|(k, v)| k == "oflags" && v.contains("post")));
    }

    #[test]
    fn test_position_pnl() {
        let position = Position {
            position_id: "pos1".to_string(),
            pair: "XBT/USD".to_string(),
            side: OrderSide::Buy,
            volume: dec!(1.0),
            entry_price: dec!(50000.00),
            mark_price: dec!(55000.00),
            unrealized_pnl: dec!(5000.00),
            realized_pnl: Decimal::ZERO,
            liquidation_price: None,
            open_time: Utc::now(),
        };

        assert_eq!(position.pnl_percent(), dec!(10)); // 10% profit
        assert!(position.is_profitable());
    }
}
