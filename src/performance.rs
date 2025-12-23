//! Performance tracking and statistics
//!
//! Track trading performance metrics like P&L, win rate, Sharpe ratio, drawdown.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A completed trade for performance tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTrade {
    pub id: String,
    pub pair: String,
    pub side: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub volume: Decimal,
    pub pnl: Decimal,
    pub pnl_percent: Decimal,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub fees: Decimal,
}

impl CompletedTrade {
    pub fn is_winner(&self) -> bool {
        self.pnl > Decimal::ZERO
    }

    pub fn duration_seconds(&self) -> i64 {
        (self.exit_time - self.entry_time).num_seconds()
    }
}

/// Performance statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    /// Total number of trades
    pub total_trades: u32,
    /// Number of winning trades
    pub winning_trades: u32,
    /// Number of losing trades
    pub losing_trades: u32,
    /// Win rate (0.0 - 1.0)
    pub win_rate: Decimal,
    /// Total P&L
    pub total_pnl: Decimal,
    /// Average P&L per trade
    pub avg_pnl: Decimal,
    /// Average winning trade P&L
    pub avg_win: Decimal,
    /// Average losing trade P&L
    pub avg_loss: Decimal,
    /// Profit factor (gross profit / gross loss)
    pub profit_factor: Decimal,
    /// Largest winning trade
    pub largest_win: Decimal,
    /// Largest losing trade
    pub largest_loss: Decimal,
    /// Maximum drawdown
    pub max_drawdown: Decimal,
    /// Maximum drawdown percentage
    pub max_drawdown_percent: Decimal,
    /// Sharpe ratio (annualized)
    pub sharpe_ratio: Decimal,
    /// Sortino ratio (annualized)
    pub sortino_ratio: Decimal,
    /// Average trade duration in seconds
    pub avg_duration_seconds: i64,
    /// Total fees paid
    pub total_fees: Decimal,
    /// Net P&L (after fees)
    pub net_pnl: Decimal,
}

/// Performance tracker
pub struct PerformanceTracker {
    trades: VecDeque<CompletedTrade>,
    equity_curve: VecDeque<EquityPoint>,
    initial_balance: Decimal,
    current_balance: Decimal,
    peak_balance: Decimal,
    max_trades: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub balance: Decimal,
    pub drawdown: Decimal,
    pub drawdown_percent: Decimal,
}

impl PerformanceTracker {
    pub fn new(initial_balance: Decimal) -> Self {
        Self {
            trades: VecDeque::new(),
            equity_curve: VecDeque::new(),
            initial_balance,
            current_balance: initial_balance,
            peak_balance: initial_balance,
            max_trades: 1000,
        }
    }

    /// Record a completed trade
    pub fn record_trade(&mut self, trade: CompletedTrade) {
        self.current_balance += trade.pnl - trade.fees;
        
        if self.current_balance > self.peak_balance {
            self.peak_balance = self.current_balance;
        }

        let drawdown = self.peak_balance - self.current_balance;
        let drawdown_percent = if self.peak_balance > Decimal::ZERO {
            drawdown / self.peak_balance * dec!(100)
        } else {
            Decimal::ZERO
        };

        self.equity_curve.push_back(EquityPoint {
            timestamp: trade.exit_time,
            balance: self.current_balance,
            drawdown,
            drawdown_percent,
        });

        self.trades.push_back(trade);

        // Keep only last N trades
        while self.trades.len() > self.max_trades {
            self.trades.pop_front();
        }
        while self.equity_curve.len() > self.max_trades {
            self.equity_curve.pop_front();
        }
    }

    /// Calculate performance statistics
    pub fn calculate_stats(&self) -> PerformanceStats {
        if self.trades.is_empty() {
            return PerformanceStats::default();
        }

        let total_trades = self.trades.len() as u32;
        let winners: Vec<_> = self.trades.iter().filter(|t| t.is_winner()).collect();
        let losers: Vec<_> = self.trades.iter().filter(|t| !t.is_winner()).collect();

        let winning_trades = winners.len() as u32;
        let losing_trades = losers.len() as u32;

        let win_rate = if total_trades > 0 {
            Decimal::from(winning_trades) / Decimal::from(total_trades)
        } else {
            Decimal::ZERO
        };

        let total_pnl: Decimal = self.trades.iter().map(|t| t.pnl).sum();
        let total_fees: Decimal = self.trades.iter().map(|t| t.fees).sum();
        let net_pnl = total_pnl - total_fees;

        let avg_pnl = total_pnl / Decimal::from(total_trades);

        let gross_profit: Decimal = winners.iter().map(|t| t.pnl).sum();
        let gross_loss: Decimal = losers.iter().map(|t| t.pnl.abs()).sum();

        let avg_win = if !winners.is_empty() {
            gross_profit / Decimal::from(winners.len())
        } else {
            Decimal::ZERO
        };

        let avg_loss = if !losers.is_empty() {
            gross_loss / Decimal::from(losers.len())
        } else {
            Decimal::ZERO
        };

        let profit_factor = if gross_loss > Decimal::ZERO {
            gross_profit / gross_loss
        } else if gross_profit > Decimal::ZERO {
            dec!(999.99) // Infinite profit factor capped
        } else {
            Decimal::ZERO
        };

        let largest_win = winners.iter().map(|t| t.pnl).max().unwrap_or(Decimal::ZERO);
        let largest_loss = losers.iter().map(|t| t.pnl.abs()).max().unwrap_or(Decimal::ZERO);

        let max_drawdown = self.equity_curve.iter()
            .map(|e| e.drawdown)
            .max()
            .unwrap_or(Decimal::ZERO);

        let max_drawdown_percent = self.equity_curve.iter()
            .map(|e| e.drawdown_percent)
            .max()
            .unwrap_or(Decimal::ZERO);

        let avg_duration_seconds = if total_trades > 0 {
            self.trades.iter().map(|t| t.duration_seconds()).sum::<i64>() / total_trades as i64
        } else {
            0
        };

        // Calculate Sharpe ratio (simplified - assumes daily returns)
        let returns: Vec<Decimal> = self.trades.iter().map(|t| t.pnl_percent).collect();
        let sharpe_ratio = calculate_sharpe(&returns);
        let sortino_ratio = calculate_sortino(&returns);

        PerformanceStats {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_pnl,
            avg_pnl,
            avg_win,
            avg_loss,
            profit_factor,
            largest_win,
            largest_loss,
            max_drawdown,
            max_drawdown_percent,
            sharpe_ratio,
            sortino_ratio,
            avg_duration_seconds,
            total_fees,
            net_pnl,
        }
    }

    /// Get equity curve for charting
    pub fn get_equity_curve(&self) -> Vec<EquityPoint> {
        self.equity_curve.iter().cloned().collect()
    }

    /// Get recent trades
    pub fn get_recent_trades(&self, count: usize) -> Vec<CompletedTrade> {
        self.trades.iter().rev().take(count).cloned().collect()
    }

    /// Get current balance
    pub fn current_balance(&self) -> Decimal {
        self.current_balance
    }

    /// Get current drawdown
    pub fn current_drawdown(&self) -> Decimal {
        self.peak_balance - self.current_balance
    }

    /// Get current drawdown percentage
    pub fn current_drawdown_percent(&self) -> Decimal {
        if self.peak_balance > Decimal::ZERO {
            (self.peak_balance - self.current_balance) / self.peak_balance * dec!(100)
        } else {
            Decimal::ZERO
        }
    }

    /// Reset tracker
    pub fn reset(&mut self, initial_balance: Decimal) {
        self.trades.clear();
        self.equity_curve.clear();
        self.initial_balance = initial_balance;
        self.current_balance = initial_balance;
        self.peak_balance = initial_balance;
    }
}

fn calculate_sharpe(returns: &[Decimal]) -> Decimal {
    if returns.len() < 2 {
        return Decimal::ZERO;
    }

    let n = Decimal::from(returns.len());
    let mean: Decimal = returns.iter().sum::<Decimal>() / n;
    
    let variance: Decimal = returns.iter()
        .map(|r| (*r - mean) * (*r - mean))
        .sum::<Decimal>() / n;

    if variance <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    // Approximate sqrt using Newton's method
    let std_dev = decimal_sqrt(variance);
    
    if std_dev <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    // Annualize (assuming daily returns, 252 trading days)
    let annualized_return = mean * dec!(252);
    let annualized_std = std_dev * decimal_sqrt(dec!(252));

    if annualized_std > Decimal::ZERO {
        annualized_return / annualized_std
    } else {
        Decimal::ZERO
    }
}

fn calculate_sortino(returns: &[Decimal]) -> Decimal {
    if returns.len() < 2 {
        return Decimal::ZERO;
    }

    let n = Decimal::from(returns.len());
    let mean: Decimal = returns.iter().sum::<Decimal>() / n;
    
    // Downside deviation (only negative returns)
    let downside_returns: Vec<Decimal> = returns.iter()
        .filter(|r| **r < Decimal::ZERO)
        .map(|r| *r * *r)
        .collect();

    if downside_returns.is_empty() {
        return dec!(999.99); // No downside = very good
    }

    let downside_variance: Decimal = downside_returns.iter().sum::<Decimal>() 
        / Decimal::from(downside_returns.len());

    let downside_dev = decimal_sqrt(downside_variance);

    if downside_dev <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    // Annualize
    let annualized_return = mean * dec!(252);
    let annualized_downside = downside_dev * decimal_sqrt(dec!(252));

    if annualized_downside > Decimal::ZERO {
        annualized_return / annualized_downside
    } else {
        Decimal::ZERO
    }
}

/// Approximate square root for Decimal using Newton's method
fn decimal_sqrt(n: Decimal) -> Decimal {
    if n <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let mut x = n;
    let two = dec!(2);
    
    for _ in 0..20 {
        let next = (x + n / x) / two;
        if (next - x).abs() < dec!(0.0000001) {
            return next;
        }
        x = next;
    }
    x
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new(dec!(10000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_tracker() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // Record a winning trade
        tracker.record_trade(CompletedTrade {
            id: "1".to_string(),
            pair: "XBT/USD".to_string(),
            side: "buy".to_string(),
            entry_price: dec!(50000),
            exit_price: dec!(51000),
            volume: dec!(0.1),
            pnl: dec!(100),
            pnl_percent: dec!(2),
            entry_time: Utc::now(),
            exit_time: Utc::now(),
            fees: dec!(1),
        });

        // Record a losing trade
        tracker.record_trade(CompletedTrade {
            id: "2".to_string(),
            pair: "XBT/USD".to_string(),
            side: "buy".to_string(),
            entry_price: dec!(51000),
            exit_price: dec!(50500),
            volume: dec!(0.1),
            pnl: dec!(-50),
            pnl_percent: dec!(-1),
            entry_time: Utc::now(),
            exit_time: Utc::now(),
            fees: dec!(1),
        });

        let stats = tracker.calculate_stats();
        assert_eq!(stats.total_trades, 2);
        assert_eq!(stats.winning_trades, 1);
        assert_eq!(stats.losing_trades, 1);
        assert_eq!(stats.win_rate, dec!(0.5));
    }

    #[test]
    fn test_decimal_sqrt() {
        let result = decimal_sqrt(dec!(4));
        assert!((result - dec!(2)).abs() < dec!(0.0001));

        let result = decimal_sqrt(dec!(9));
        assert!((result - dec!(3)).abs() < dec!(0.0001));
    }
}
