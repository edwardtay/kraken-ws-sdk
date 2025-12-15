//! Retry policies and strategies for SDK operations
//!
//! Provides configurable retry behavior with exponential backoff, jitter,
//! and circuit breaker patterns.

use std::time::Duration;
use rand::Rng;

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier (e.g., 2.0 for exponential)
    pub backoff_multiplier: f64,
    /// Whether to add jitter to delays
    pub jitter: bool,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
    /// Retryable error codes
    pub retryable_errors: Vec<RetryableError>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
            jitter_factor: 0.25,
            retryable_errors: vec![
                RetryableError::Timeout,
                RetryableError::ConnectionReset,
                RetryableError::ServiceUnavailable,
                RetryableError::RateLimited,
            ],
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy builder
    pub fn builder() -> RetryPolicyBuilder {
        RetryPolicyBuilder::new()
    }

    /// No retries
    pub fn none() -> Self {
        Self {
            max_attempts: 0,
            ..Default::default()
        }
    }

    /// Aggressive retry for critical operations
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.5,
            jitter: true,
            jitter_factor: 0.3,
            retryable_errors: vec![
                RetryableError::Timeout,
                RetryableError::ConnectionReset,
                RetryableError::ServiceUnavailable,
                RetryableError::RateLimited,
                RetryableError::InternalError,
            ],
        }
    }

    /// Conservative retry for non-critical operations
    pub fn conservative() -> Self {
        Self {
            max_attempts: 2,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: true,
            jitter_factor: 0.1,
            retryable_errors: vec![
                RetryableError::Timeout,
                RetryableError::ServiceUnavailable,
            ],
        }
    }

    /// Calculate delay for a given attempt number
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32 - 1);
        
        let capped_delay = base_delay.min(self.max_delay.as_millis() as f64);
        
        let final_delay = if self.jitter {
            let jitter_range = capped_delay * self.jitter_factor;
            let jitter = rand::thread_rng().gen_range(-jitter_range..jitter_range);
            (capped_delay + jitter).max(0.0)
        } else {
            capped_delay
        };

        Duration::from_millis(final_delay as u64)
    }

    /// Check if an error is retryable
    pub fn is_retryable(&self, error: &RetryableError) -> bool {
        self.retryable_errors.contains(error)
    }

    /// Check if we should retry given the attempt count
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

/// Builder for RetryPolicy
pub struct RetryPolicyBuilder {
    policy: RetryPolicy,
}

impl RetryPolicyBuilder {
    pub fn new() -> Self {
        Self {
            policy: RetryPolicy::default(),
        }
    }

    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.policy.max_attempts = attempts;
        self
    }

    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.policy.initial_delay = delay;
        self
    }

    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.policy.max_delay = delay;
        self
    }

    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.policy.backoff_multiplier = multiplier;
        self
    }

    pub fn with_jitter(mut self, enabled: bool) -> Self {
        self.policy.jitter = enabled;
        self
    }

    pub fn jitter_factor(mut self, factor: f64) -> Self {
        self.policy.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }

    pub fn retryable_errors(mut self, errors: Vec<RetryableError>) -> Self {
        self.policy.retryable_errors = errors;
        self
    }

    pub fn build(self) -> RetryPolicy {
        self.policy
    }
}

impl Default for RetryPolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can be retried
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RetryableError {
    Timeout,
    ConnectionReset,
    ServiceUnavailable,
    RateLimited,
    InternalError,
    NetworkError,
}

/// Retry executor that handles the retry logic
pub struct RetryExecutor {
    policy: RetryPolicy,
    current_attempt: u32,
}

impl RetryExecutor {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            current_attempt: 0,
        }
    }

    /// Execute an async operation with retries
    pub async fn execute<F, Fut, T, E>(&mut self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: Into<RetryableError> + Clone,
    {
        loop {
            self.current_attempt += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let retryable: RetryableError = e.clone().into();
                    
                    if !self.policy.is_retryable(&retryable) || 
                       !self.policy.should_retry(self.current_attempt) {
                        return Err(e);
                    }

                    let delay = self.policy.calculate_delay(self.current_attempt);
                    tracing::warn!(
                        "Operation failed (attempt {}/{}), retrying in {:?}",
                        self.current_attempt,
                        self.policy.max_attempts,
                        delay
                    );
                    
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Get current attempt number
    pub fn current_attempt(&self) -> u32 {
        self.current_attempt
    }

    /// Reset the executor for reuse
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Duration to keep circuit open
    pub reset_timeout: Duration,
    /// Current failure count
    failure_count: u32,
    /// Circuit state
    state: CircuitState,
    /// Time when circuit was opened
    opened_at: Option<std::time::Instant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            reset_timeout,
            failure_count: 0,
            state: CircuitState::Closed,
            opened_at: None,
        }
    }

    /// Check if request should be allowed
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(opened_at) = self.opened_at {
                    if opened_at.elapsed() >= self.reset_timeout {
                        self.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.opened_at = None;
    }

    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        
        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
            self.opened_at = Some(std::time::Instant::now());
        }
    }

    /// Get current state
    pub fn state(&self) -> &CircuitState {
        &self.state
    }

    /// Get failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_delay_calculation() {
        let policy = RetryPolicy {
            jitter: false,
            ..Default::default()
        };

        assert_eq!(policy.calculate_delay(0), Duration::ZERO);
        assert_eq!(policy.calculate_delay(1), Duration::from_millis(100));
        assert_eq!(policy.calculate_delay(2), Duration::from_millis(200));
        assert_eq!(policy.calculate_delay(3), Duration::from_millis(400));
    }

    #[test]
    fn test_circuit_breaker() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(1));
        
        assert!(cb.allow_request());
        cb.record_failure();
        assert!(cb.allow_request());
        cb.record_failure();
        assert!(cb.allow_request());
        cb.record_failure();
        
        // Circuit should be open now
        assert!(!cb.allow_request());
        assert_eq!(cb.state(), &CircuitState::Open);
    }
}
