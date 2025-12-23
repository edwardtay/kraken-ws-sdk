//! Batch order operations for Kraken
//!
//! Provides atomic batch order placement and OCO (one-cancels-other) orders.

use crate::trading::{OrderRequest, OrderResponse, OrderSide, CancelResponse};
use crate::error::SdkError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Batch order request - multiple orders submitted together
#[derive(Debug, Clone)]
pub struct BatchOrderRequest {
    pub orders: Vec<OrderRequest>,
    /// If true, fail all if any fails
    pub atomic: bool,
}

impl BatchOrderRequest {
    pub fn new() -> Self {
        Self {
            orders: Vec::new(),
            atomic: true,
        }
    }

    pub fn add(mut self, order: OrderRequest) -> Self {
        self.orders.push(order);
        self
    }

    pub fn non_atomic(mut self) -> Self {
        self.atomic = false;
        self
    }
}

/// Batch order result
#[derive(Debug, Clone)]
pub struct BatchOrderResult {
    pub successful: Vec<OrderResponse>,
    pub failed: Vec<BatchOrderError>,
    pub all_succeeded: bool,
}

#[derive(Debug, Clone)]
pub struct BatchOrderError {
    pub index: usize,
    pub order: OrderRequest,
    pub error: String,
}

/// OCO (One-Cancels-Other) order pair
/// When one order fills, the other is automatically cancelled
#[derive(Debug, Clone, Serialize)]
pub struct OcoOrder {
    pub pair: String,
    pub side: OrderSide,
    pub volume: Decimal,
    /// Primary order (e.g., limit buy)
    pub primary_price: Decimal,
    /// Secondary order (e.g., stop loss)
    pub secondary_price: Decimal,
    /// Client ID prefix for tracking
    pub client_id_prefix: Option<String>,
}

impl OcoOrder {
    /// Create a buy OCO: limit buy + stop loss above
    pub fn buy_with_stop(pair: &str, volume: Decimal, limit_price: Decimal, stop_price: Decimal) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Buy,
            volume,
            primary_price: limit_price,
            secondary_price: stop_price,
            client_id_prefix: None,
        }
    }

    /// Create a sell OCO: limit sell + stop loss below
    pub fn sell_with_stop(pair: &str, volume: Decimal, limit_price: Decimal, stop_price: Decimal) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Sell,
            volume,
            primary_price: limit_price,
            secondary_price: stop_price,
            client_id_prefix: None,
        }
    }

    pub fn with_client_id(mut self, prefix: &str) -> Self {
        self.client_id_prefix = Some(prefix.to_string());
        self
    }
}

/// OCO order result
#[derive(Debug, Clone)]
pub struct OcoOrderResult {
    pub primary_txid: String,
    pub secondary_txid: String,
    pub oco_id: String,
}

/// Bracket order - entry + take profit + stop loss
#[derive(Debug, Clone, Serialize)]
pub struct BracketOrder {
    pub pair: String,
    pub side: OrderSide,
    pub volume: Decimal,
    /// Entry price (limit order)
    pub entry_price: Decimal,
    /// Take profit price
    pub take_profit_price: Decimal,
    /// Stop loss price
    pub stop_loss_price: Decimal,
    pub client_id_prefix: Option<String>,
}

impl BracketOrder {
    /// Create a long bracket: buy entry + sell TP + sell SL
    pub fn long(
        pair: &str,
        volume: Decimal,
        entry: Decimal,
        take_profit: Decimal,
        stop_loss: Decimal,
    ) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Buy,
            volume,
            entry_price: entry,
            take_profit_price: take_profit,
            stop_loss_price: stop_loss,
            client_id_prefix: None,
        }
    }

    /// Create a short bracket: sell entry + buy TP + buy SL
    pub fn short(
        pair: &str,
        volume: Decimal,
        entry: Decimal,
        take_profit: Decimal,
        stop_loss: Decimal,
    ) -> Self {
        Self {
            pair: pair.to_string(),
            side: OrderSide::Sell,
            volume,
            entry_price: entry,
            take_profit_price: take_profit,
            stop_loss_price: stop_loss,
            client_id_prefix: None,
        }
    }

    pub fn with_client_id(mut self, prefix: &str) -> Self {
        self.client_id_prefix = Some(prefix.to_string());
        self
    }
}

/// Bracket order result
#[derive(Debug, Clone)]
pub struct BracketOrderResult {
    pub entry_txid: String,
    pub take_profit_txid: String,
    pub stop_loss_txid: String,
    pub bracket_id: String,
}

/// Position sizing helpers
pub mod sizing {
    use rust_decimal::Decimal;

    /// Calculate position size based on percentage of balance
    pub fn percent_of_balance(balance: Decimal, percent: Decimal) -> Decimal {
        balance * percent / Decimal::from(100)
    }

    /// Calculate position size based on risk amount and stop distance
    pub fn risk_based(
        risk_amount: Decimal,
        entry_price: Decimal,
        stop_price: Decimal,
    ) -> Decimal {
        let stop_distance = (entry_price - stop_price).abs();
        if stop_distance.is_zero() {
            return Decimal::ZERO;
        }
        risk_amount / stop_distance
    }

    /// Calculate position size for fixed dollar amount
    pub fn fixed_notional(notional: Decimal, price: Decimal) -> Decimal {
        if price.is_zero() {
            return Decimal::ZERO;
        }
        notional / price
    }

    /// Round to valid lot size
    pub fn round_to_lot(size: Decimal, lot_size: Decimal) -> Decimal {
        if lot_size.is_zero() {
            return size;
        }
        (size / lot_size).floor() * lot_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_batch_order_builder() {
        let batch = BatchOrderRequest::new()
            .add(OrderRequest::limit_buy("XBT/USD", dec!(0.001), dec!(50000)))
            .add(OrderRequest::limit_sell("XBT/USD", dec!(0.001), dec!(55000)));
        
        assert_eq!(batch.orders.len(), 2);
        assert!(batch.atomic);
    }

    #[test]
    fn test_oco_order() {
        let oco = OcoOrder::buy_with_stop("XBT/USD", dec!(0.01), dec!(50000), dec!(48000));
        assert_eq!(oco.primary_price, dec!(50000));
        assert_eq!(oco.secondary_price, dec!(48000));
    }

    #[test]
    fn test_bracket_order() {
        let bracket = BracketOrder::long(
            "XBT/USD",
            dec!(0.01),
            dec!(50000),  // entry
            dec!(55000),  // TP
            dec!(48000),  // SL
        );
        assert_eq!(bracket.entry_price, dec!(50000));
        assert_eq!(bracket.take_profit_price, dec!(55000));
        assert_eq!(bracket.stop_loss_price, dec!(48000));
    }

    #[test]
    fn test_percent_sizing() {
        let size = sizing::percent_of_balance(dec!(10000), dec!(10));
        assert_eq!(size, dec!(1000));
    }

    #[test]
    fn test_risk_based_sizing() {
        // Risk $100, entry $50000, stop $49000 = $1000 distance
        // Size = $100 / $1000 = 0.1 BTC
        let size = sizing::risk_based(dec!(100), dec!(50000), dec!(49000));
        assert_eq!(size, dec!(0.1));
    }
}
