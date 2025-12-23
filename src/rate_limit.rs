//! Rate limiting for Kraken API
//!
//! Implements Kraken's tier-based rate limiting:
//! https://docs.kraken.com/rest/#section/Rate-Limits
//!
//! - Starter: 15 counter, decays 0.33/sec
//! - Intermediate: 20 counter, decays 0.5/sec  
//! - Pro: 20 counter, decays 1.0/sec

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use crate::error::SdkError;

/// Kraken account tier for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountTier {
    Starter,
    Intermediate,
    Pro,
}

impl AccountTier {
    /// Maximum counter value for this tier
    pub fn max_counter(&self) -> u32 {
        match self {
            AccountTier::Starter => 15,
            AccountTier::Intermediate | AccountTier::Pro => 20,
        }
    }

    /// Counter decay rate per second
    pub fn decay_rate(&self) -> f64 {
        match self {
            AccountTier::Starter => 0.33,
            AccountTier::Intermediate => 0.5,
            AccountTier::Pro => 1.0,
        }
    }
}

impl Default for AccountTier {
    fn default() -> Self {
        AccountTier::Starter // Conservative default
    }
}

/// API endpoint categories with different costs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointCost {
    /// Ledger/trade history queries (2 points)
    Ledger,
    /// Add/cancel order (0 points, but matching engine limits apply)
    Order,
    /// Standard queries (1 point)
    Standard,
}

impl EndpointCost {
    pub fn cost(&self) -> u32 {
        match self {
            EndpointCost::Ledger => 2,
            EndpointCost::Order => 0, // Orders have separate matching engine limits
            EndpointCost::Standard => 1,
        }
    }
}

/// Rate limiter for Kraken API requests
pub struct RateLimiter {
    tier: AccountTier,
    counter: AtomicU32,
    last_update: Mutex<Instant>,
    /// Matching engine rate limit (orders per second)
    order_counter: AtomicU32,
    last_order_reset: Mutex<Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter for the given account tier
    pub fn new(tier: AccountTier) -> Self {
        Self {
            tier,
            counter: AtomicU32::new(0),
            last_update: Mutex::new(Instant::now()),
            order_counter: AtomicU32::new(0),
            last_order_reset: Mutex::new(Instant::now()),
        }
    }

    /// Get current counter value after applying decay
    pub fn current_counter(&self) -> u32 {
        self.apply_decay();
        self.counter.load(Ordering::Relaxed)
    }

    /// Get remaining capacity before hitting limit
    pub fn remaining(&self) -> u32 {
        self.tier.max_counter().saturating_sub(self.current_counter())
    }

    /// Check if we can make a request without blocking
    pub fn can_request(&self, cost: EndpointCost) -> bool {
        self.remaining() >= cost.cost()
    }

    /// Acquire rate limit capacity, blocking if necessary
    ///
    /// Returns Ok(()) when the request can proceed, or Err if interrupted
    pub async fn acquire(&self, cost: EndpointCost) -> Result<(), SdkError> {
        let cost_value = cost.cost();
        
        // Orders have separate limits
        if cost == EndpointCost::Order {
            return self.acquire_order_slot().await;
        }

        loop {
            self.apply_decay();
            
            let current = self.counter.load(Ordering::Relaxed);
            let new_value = current + cost_value;
            
            if new_value <= self.tier.max_counter() {
                // Try to atomically increment
                if self.counter.compare_exchange(
                    current,
                    new_value,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ).is_ok() {
                    tracing::trace!(
                        "Rate limit acquired: {} -> {} / {}",
                        current, new_value, self.tier.max_counter()
                    );
                    return Ok(());
                }
                // CAS failed, retry
                continue;
            }

            // Need to wait for decay
            let wait_time = self.time_until_available(cost_value);
            tracing::debug!(
                "Rate limit reached ({}/{}), waiting {:?}",
                current, self.tier.max_counter(), wait_time
            );
            sleep(wait_time).await;
        }
    }

    /// Acquire an order slot (matching engine limit)
    async fn acquire_order_slot(&self) -> Result<(), SdkError> {
        // Kraken allows ~60 orders per minute for most tiers
        const MAX_ORDERS_PER_SECOND: u32 = 1;
        
        loop {
            let mut last_reset = self.last_order_reset.lock().unwrap();
            let elapsed = last_reset.elapsed();
            
            if elapsed >= Duration::from_secs(1) {
                // Reset counter every second
                self.order_counter.store(0, Ordering::Relaxed);
                *last_reset = Instant::now();
            }
            drop(last_reset);

            let current = self.order_counter.load(Ordering::Relaxed);
            if current < MAX_ORDERS_PER_SECOND {
                if self.order_counter.compare_exchange(
                    current,
                    current + 1,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ).is_ok() {
                    return Ok(());
                }
                continue;
            }

            // Wait until next second
            let last_reset = self.last_order_reset.lock().unwrap();
            let wait_time = Duration::from_secs(1).saturating_sub(last_reset.elapsed());
            drop(last_reset);
            
            if wait_time > Duration::ZERO {
                sleep(wait_time).await;
            }
        }
    }

    /// Apply decay based on elapsed time
    fn apply_decay(&self) {
        let mut last_update = self.last_update.lock().unwrap();
        let elapsed = last_update.elapsed();
        
        if elapsed.as_millis() < 100 {
            return; // Don't decay too frequently
        }

        let decay_amount = (elapsed.as_secs_f64() * self.tier.decay_rate()) as u32;
        if decay_amount > 0 {
            let current = self.counter.load(Ordering::Relaxed);
            let new_value = current.saturating_sub(decay_amount);
            self.counter.store(new_value, Ordering::Relaxed);
            *last_update = Instant::now();
            
            tracing::trace!(
                "Rate limit decay: {} -> {} (decayed {})",
                current, new_value, decay_amount
            );
        }
    }

    /// Calculate time until we have enough capacity
    fn time_until_available(&self, needed: u32) -> Duration {
        let current = self.counter.load(Ordering::Relaxed);
        let excess = current.saturating_sub(self.tier.max_counter().saturating_sub(needed));
        
        if excess == 0 {
            return Duration::ZERO;
        }

        let seconds_needed = excess as f64 / self.tier.decay_rate();
        Duration::from_secs_f64(seconds_needed + 0.1) // Add small buffer
    }

    /// Get rate limiter statistics
    pub fn stats(&self) -> RateLimitStats {
        self.apply_decay();
        RateLimitStats {
            tier: self.tier,
            current_counter: self.counter.load(Ordering::Relaxed),
            max_counter: self.tier.max_counter(),
            remaining: self.remaining(),
            decay_rate: self.tier.decay_rate(),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(AccountTier::default())
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub tier: AccountTier,
    pub current_counter: u32,
    pub max_counter: u32,
    pub remaining: u32,
    pub decay_rate: f64,
}

impl std::fmt::Display for RateLimitStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RateLimit[{:?}]: {}/{} (remaining: {}, decay: {}/s)",
            self.tier, self.current_counter, self.max_counter, self.remaining, self.decay_rate
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_defaults() {
        assert_eq!(AccountTier::Starter.max_counter(), 15);
        assert_eq!(AccountTier::Pro.max_counter(), 20);
    }

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(AccountTier::Starter);
        assert_eq!(limiter.remaining(), 15);
    }

    #[tokio::test]
    async fn test_acquire_decrements_counter() {
        let limiter = RateLimiter::new(AccountTier::Pro);
        
        limiter.acquire(EndpointCost::Standard).await.unwrap();
        assert_eq!(limiter.current_counter(), 1);
        
        limiter.acquire(EndpointCost::Ledger).await.unwrap();
        assert_eq!(limiter.current_counter(), 3); // 1 + 2
    }

    #[test]
    fn test_can_request() {
        let limiter = RateLimiter::new(AccountTier::Starter);
        assert!(limiter.can_request(EndpointCost::Standard));
        assert!(limiter.can_request(EndpointCost::Ledger));
    }
}
