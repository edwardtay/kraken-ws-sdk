//! Deterministic connection state machine
//!
//! This module provides a formal state machine for connection management.
//! Each state has explicit transitions with single causes and actions.
//!
//! ## State Diagram
//!
//! ```text
//! DISCONNECTED ──connect()──▶ CONNECTING ──success──▶ AUTHENTICATING
//!      ▲                          │                        │
//!      │                       failure                  success/skip
//!      │                          │                        ▼
//!      │                          ▼                   SUBSCRIBING
//!      │                      DEGRADED                     │
//!      │                          │                     success
//!      │                       retry                       ▼
//!      │                          │                   SUBSCRIBED ◀──resync_complete──┐
//!      │                          ▼                        │                         │
//!      └────────close()────── CLOSED ◀──max_retries────────┴───gap_detected──▶ RESYNCING
//! ```

use std::fmt;
use std::time::{Duration, Instant};

/// Connection state machine states
/// 
/// Each state represents a distinct phase in the connection lifecycle.
/// Transitions are deterministic - each state has defined exit conditions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    /// Initial state - not connected
    /// 
    /// Transitions:
    /// - `connect()` → CONNECTING
    Disconnected,
    
    /// Attempting to establish WebSocket connection
    /// 
    /// Transitions:
    /// - success → AUTHENTICATING (if auth required) or SUBSCRIBING
    /// - failure → DEGRADED
    /// - timeout → DEGRADED
    Connecting,
    
    /// Authenticating with API credentials
    /// 
    /// Transitions:
    /// - success → SUBSCRIBING
    /// - failure → DEGRADED (auth error)
    /// - timeout → DEGRADED
    Authenticating,
    
    /// Sending subscription requests
    /// 
    /// Transitions:
    /// - all subscribed → SUBSCRIBED
    /// - partial failure → DEGRADED
    /// - timeout → DEGRADED
    Subscribing,
    
    /// Fully connected and receiving data
    /// 
    /// Transitions:
    /// - disconnect → DEGRADED
    /// - gap detected → RESYNCING
    /// - close() → CLOSED
    Subscribed,
    
    /// Resyncing after sequence gap or reconnect
    /// 
    /// Transitions:
    /// - resync complete → SUBSCRIBED
    /// - failure → DEGRADED
    /// - timeout → DEGRADED
    Resyncing,
    
    /// Connection degraded - attempting recovery
    /// 
    /// Transitions:
    /// - retry → CONNECTING
    /// - max retries → CLOSED
    /// - close() → CLOSED
    Degraded {
        /// Reason for degradation
        reason: DegradedReason,
        /// Number of retry attempts
        retry_count: u32,
        /// When degradation started
        since: Instant,
    },
    
    /// Connection closed - terminal state
    /// 
    /// Transitions:
    /// - connect() → CONNECTING (new connection)
    Closed {
        /// Reason for closure
        reason: ClosedReason,
    },
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Disconnected
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "DISCONNECTED"),
            ConnectionState::Connecting => write!(f, "CONNECTING"),
            ConnectionState::Authenticating => write!(f, "AUTHENTICATING"),
            ConnectionState::Subscribing => write!(f, "SUBSCRIBING"),
            ConnectionState::Subscribed => write!(f, "SUBSCRIBED"),
            ConnectionState::Resyncing => write!(f, "RESYNCING"),
            ConnectionState::Degraded { reason, retry_count, .. } => {
                write!(f, "DEGRADED({:?}, retries={})", reason, retry_count)
            }
            ConnectionState::Closed { reason } => write!(f, "CLOSED({:?})", reason),
        }
    }
}

/// Reasons for entering DEGRADED state
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DegradedReason {
    /// Network connection failed
    ConnectionFailed,
    /// Connection timeout
    Timeout,
    /// Authentication failed
    AuthenticationFailed,
    /// Subscription failed
    SubscriptionFailed,
    /// Server closed connection
    ServerDisconnect,
    /// Sequence gap detected
    SequenceGap { expected: u64, received: u64 },
    /// Checksum mismatch in order book
    ChecksumMismatch,
    /// Heartbeat timeout
    HeartbeatTimeout,
}

/// Reasons for entering CLOSED state
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClosedReason {
    /// User requested close
    UserRequested,
    /// Maximum retry attempts exceeded
    MaxRetriesExceeded,
    /// Unrecoverable authentication error
    AuthenticationError,
    /// Server rejected connection permanently
    ServerRejected,
}

/// State transition event
/// 
/// Emitted whenever the state machine transitions between states.
/// Use this for logging, metrics, and debugging.
#[derive(Debug, Clone)]
pub struct StateTransition {
    /// Previous state
    pub from: ConnectionState,
    /// New state
    pub to: ConnectionState,
    /// What triggered the transition
    pub trigger: TransitionTrigger,
    /// When the transition occurred
    pub timestamp: Instant,
}

impl StateTransition {
    /// Create a new state transition
    pub fn new(from: ConnectionState, to: ConnectionState, trigger: TransitionTrigger) -> Self {
        Self {
            from,
            to,
            trigger,
            timestamp: Instant::now(),
        }
    }
}

/// What triggered a state transition
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTrigger {
    /// User called connect()
    UserConnect,
    /// User called close()
    UserClose,
    /// WebSocket connection established
    ConnectionEstablished,
    /// WebSocket connection failed
    ConnectionFailed(String),
    /// Authentication succeeded
    AuthSuccess,
    /// Authentication failed
    AuthFailed(String),
    /// All subscriptions confirmed
    SubscriptionsConfirmed,
    /// Subscription failed
    SubscriptionFailed(String),
    /// Server disconnected
    ServerDisconnect(u16, String),
    /// Heartbeat timeout
    HeartbeatTimeout,
    /// Sequence gap detected
    SequenceGap { expected: u64, received: u64 },
    /// Resync completed
    ResyncComplete,
    /// Retry attempt
    RetryAttempt(u32),
    /// Max retries exceeded
    MaxRetriesExceeded,
}

/// Connection state machine
/// 
/// Manages connection lifecycle with deterministic transitions.
pub struct StateMachine {
    state: ConnectionState,
    config: StateMachineConfig,
    transition_history: Vec<StateTransition>,
    max_history: usize,
    /// Current retry count (persists across state transitions)
    current_retry_count: u32,
}

/// Configuration for the state machine
#[derive(Debug, Clone)]
pub struct StateMachineConfig {
    /// Maximum retry attempts before giving up
    pub max_retries: u32,
    /// Initial retry delay
    pub initial_retry_delay: Duration,
    /// Maximum retry delay
    pub max_retry_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Heartbeat timeout
    pub heartbeat_timeout: Duration,
    /// Whether authentication is required
    pub requires_auth: bool,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(10),
            requires_auth: false,
        }
    }
}

impl StateMachine {
    /// Create a new state machine
    pub fn new(config: StateMachineConfig) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            config,
            transition_history: Vec::new(),
            max_history: 100,
            current_retry_count: 0,
        }
    }
    
    /// Get current state
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }
    
    /// Check if in a connected state (can receive data)
    pub fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Subscribed | ConnectionState::Resyncing)
    }
    
    /// Check if in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self.state, ConnectionState::Closed { .. })
    }
    
    /// Get transition history
    pub fn history(&self) -> &[StateTransition] {
        &self.transition_history
    }
    
    /// Transition to a new state
    fn transition(&mut self, to: ConnectionState, trigger: TransitionTrigger) -> StateTransition {
        let from = std::mem::replace(&mut self.state, to.clone());
        let transition = StateTransition::new(from, to, trigger);
        
        // Keep history bounded
        if self.transition_history.len() >= self.max_history {
            self.transition_history.remove(0);
        }
        self.transition_history.push(transition.clone());
        
        tracing::info!("State transition: {} -> {} ({:?})", 
            transition.from, transition.to, transition.trigger);
        
        transition
    }
    
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // STATE TRANSITION METHODS
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    
    /// User initiates connection
    pub fn connect(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Disconnected | ConnectionState::Closed { .. } => {
                Ok(self.transition(ConnectionState::Connecting, TransitionTrigger::UserConnect))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "connect".to_string(),
            }),
        }
    }
    
    /// User requests close
    pub fn close(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Closed { .. } => Err(StateError::AlreadyClosed),
            _ => Ok(self.transition(
                ConnectionState::Closed { reason: ClosedReason::UserRequested },
                TransitionTrigger::UserClose,
            )),
        }
    }
    
    /// WebSocket connection established
    pub fn connection_established(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Connecting => {
                // Reset retry count on successful connection
                self.current_retry_count = 0;
                let next = if self.config.requires_auth {
                    ConnectionState::Authenticating
                } else {
                    ConnectionState::Subscribing
                };
                Ok(self.transition(next, TransitionTrigger::ConnectionEstablished))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "connection_established".to_string(),
            }),
        }
    }
    
    /// WebSocket connection failed
    pub fn connection_failed(&mut self, error: String) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Connecting => {
                Ok(self.transition(
                    ConnectionState::Degraded {
                        reason: DegradedReason::ConnectionFailed,
                        retry_count: self.current_retry_count,
                        since: Instant::now(),
                    },
                    TransitionTrigger::ConnectionFailed(error),
                ))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "connection_failed".to_string(),
            }),
        }
    }
    
    /// Authentication succeeded
    pub fn auth_success(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Authenticating => {
                Ok(self.transition(ConnectionState::Subscribing, TransitionTrigger::AuthSuccess))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "auth_success".to_string(),
            }),
        }
    }
    
    /// Authentication failed
    pub fn auth_failed(&mut self, error: String) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Authenticating => {
                Ok(self.transition(
                    ConnectionState::Degraded {
                        reason: DegradedReason::AuthenticationFailed,
                        retry_count: 0,
                        since: Instant::now(),
                    },
                    TransitionTrigger::AuthFailed(error),
                ))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "auth_failed".to_string(),
            }),
        }
    }
    
    /// All subscriptions confirmed
    pub fn subscriptions_confirmed(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Subscribing => {
                Ok(self.transition(ConnectionState::Subscribed, TransitionTrigger::SubscriptionsConfirmed))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "subscriptions_confirmed".to_string(),
            }),
        }
    }
    
    /// Sequence gap detected
    pub fn gap_detected(&mut self, expected: u64, received: u64) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Subscribed => {
                Ok(self.transition(
                    ConnectionState::Resyncing,
                    TransitionTrigger::SequenceGap { expected, received },
                ))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "gap_detected".to_string(),
            }),
        }
    }
    
    /// Resync completed
    pub fn resync_complete(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Resyncing => {
                Ok(self.transition(ConnectionState::Subscribed, TransitionTrigger::ResyncComplete))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "resync_complete".to_string(),
            }),
        }
    }
    
    /// Server disconnected
    pub fn server_disconnect(&mut self, code: u16, reason: String) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Subscribed | ConnectionState::Resyncing | 
            ConnectionState::Subscribing | ConnectionState::Authenticating => {
                Ok(self.transition(
                    ConnectionState::Degraded {
                        reason: DegradedReason::ServerDisconnect,
                        retry_count: 0,
                        since: Instant::now(),
                    },
                    TransitionTrigger::ServerDisconnect(code, reason),
                ))
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "server_disconnect".to_string(),
            }),
        }
    }
    
    /// Attempt retry from degraded state
    pub fn retry(&mut self) -> Result<StateTransition, StateError> {
        match &self.state {
            ConnectionState::Degraded { .. } => {
                self.current_retry_count += 1;
                if self.current_retry_count > self.config.max_retries {
                    Ok(self.transition(
                        ConnectionState::Closed { reason: ClosedReason::MaxRetriesExceeded },
                        TransitionTrigger::MaxRetriesExceeded,
                    ))
                } else {
                    Ok(self.transition(
                        ConnectionState::Connecting,
                        TransitionTrigger::RetryAttempt(self.current_retry_count),
                    ))
                }
            }
            _ => Err(StateError::InvalidTransition {
                from: self.state.clone(),
                action: "retry".to_string(),
            }),
        }
    }
    
    /// Calculate retry delay with exponential backoff
    pub fn retry_delay(&self) -> Duration {
        let delay = self.config.initial_retry_delay.as_millis() as f64
            * self.config.backoff_multiplier.powi(self.current_retry_count as i32);
        Duration::from_millis(delay.min(self.config.max_retry_delay.as_millis() as f64) as u64)
    }
    
    /// Get current retry count
    pub fn retry_count(&self) -> u32 {
        self.current_retry_count
    }
}

/// State machine errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// Invalid state transition attempted
    InvalidTransition {
        from: ConnectionState,
        action: String,
    },
    /// Connection already closed
    AlreadyClosed,
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::InvalidTransition { from, action } => {
                write!(f, "Invalid transition: cannot {} from state {}", action, from)
            }
            StateError::AlreadyClosed => write!(f, "Connection already closed"),
        }
    }
}

impl std::error::Error for StateError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_happy_path() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        
        assert_eq!(*sm.state(), ConnectionState::Disconnected);
        
        sm.connect().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Connecting));
        
        sm.connection_established().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Subscribing));
        
        sm.subscriptions_confirmed().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Subscribed));
        assert!(sm.is_connected());
        
        sm.close().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Closed { .. }));
        assert!(sm.is_terminal());
    }
    
    #[test]
    fn test_retry_with_backoff() {
        let mut sm = StateMachine::new(StateMachineConfig {
            max_retries: 3,
            ..Default::default()
        });
        
        sm.connect().unwrap();
        sm.connection_failed("network error".to_string()).unwrap();
        
        assert!(matches!(*sm.state(), ConnectionState::Degraded { .. }));
        
        // Retry 1
        sm.retry().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Connecting));
        sm.connection_failed("still failing".to_string()).unwrap();
        
        // Retry 2
        sm.retry().unwrap();
        sm.connection_failed("still failing".to_string()).unwrap();
        
        // Retry 3
        sm.retry().unwrap();
        sm.connection_failed("still failing".to_string()).unwrap();
        
        // Retry 4 - should exceed max
        sm.retry().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Closed { reason: ClosedReason::MaxRetriesExceeded }));
    }
    
    #[test]
    fn test_gap_detection_and_resync() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        
        sm.connect().unwrap();
        sm.connection_established().unwrap();
        sm.subscriptions_confirmed().unwrap();
        
        // Gap detected
        sm.gap_detected(100, 105).unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Resyncing));
        
        // Resync complete
        sm.resync_complete().unwrap();
        assert!(matches!(*sm.state(), ConnectionState::Subscribed));
    }
    
    #[test]
    fn test_invalid_transitions() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        
        // Can't establish connection when disconnected
        assert!(sm.connection_established().is_err());
        
        // Can't resync when not subscribed
        assert!(sm.resync_complete().is_err());
    }
}
