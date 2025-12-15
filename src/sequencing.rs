//! Deterministic Message Sequencing
//! 
//! Production-grade sequence number validation and gap detection
//! for reliable trading infrastructure.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Sequence tracking state for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceState {
    /// Last successfully processed sequence number
    pub last_sequence: u64,
    /// Whether a gap was detected
    pub gap_detected: bool,
    /// Whether a resync was triggered
    pub resync_triggered: bool,
    /// Number of gaps detected since start
    pub total_gaps: u64,
    /// Number of messages processed
    pub messages_processed: u64,
    /// Last update timestamp
    pub last_update: DateTime<Utc>,
    /// Pending out-of-order messages (seq -> message)
    #[serde(skip)]
    pending_messages: HashMap<u64, PendingMessage>,
}

/// A message waiting to be processed in order
#[derive(Debug, Clone)]
struct PendingMessage {
    sequence: u64,
    data: String,
    received_at: DateTime<Utc>,
}

impl Default for SequenceState {
    fn default() -> Self {
        Self {
            last_sequence: 0,
            gap_detected: false,
            resync_triggered: false,
            total_gaps: 0,
            messages_processed: 0,
            last_update: Utc::now(),
            pending_messages: HashMap::new(),
        }
    }
}

/// Result of sequence validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceResult {
    /// Whether the message is in order
    pub in_order: bool,
    /// The sequence number of this message
    pub sequence: u64,
    /// Expected sequence number
    pub expected: u64,
    /// Gap size (0 if no gap)
    pub gap_size: u64,
    /// Whether this triggered a resync
    pub resync_triggered: bool,
    /// Current state after processing
    pub state: SequenceState,
}

/// Callback for gap detection events
pub type GapCallback = Arc<dyn Fn(GapEvent) + Send + Sync>;

/// Callback for resync events
pub type ResyncCallback = Arc<dyn Fn(ResyncEvent) + Send + Sync>;

/// Gap detection event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapEvent {
    pub channel: String,
    pub expected_sequence: u64,
    pub received_sequence: u64,
    pub gap_size: u64,
    pub timestamp: DateTime<Utc>,
}

/// Resync event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResyncEvent {
    pub channel: String,
    pub last_good_sequence: u64,
    pub reason: ResyncReason,
    pub timestamp: DateTime<Utc>,
}

/// Reason for resync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResyncReason {
    GapTooLarge { gap_size: u64 },
    TooManyPending { count: usize },
    Timeout { seconds: u64 },
    ManualRequest,
    ConnectionReset,
}

/// Configuration for sequence manager
#[derive(Debug, Clone)]
pub struct SequenceConfig {
    /// Maximum gap size before triggering resync
    pub max_gap_size: u64,
    /// Maximum pending messages before triggering resync
    pub max_pending_messages: usize,
    /// Timeout for pending messages (seconds)
    pub pending_timeout_secs: u64,
    /// Whether to auto-resync on large gaps
    pub auto_resync: bool,
}

impl Default for SequenceConfig {
    fn default() -> Self {
        Self {
            max_gap_size: 100,
            max_pending_messages: 1000,
            pending_timeout_secs: 30,
            auto_resync: true,
        }
    }
}

/// Production-grade sequence manager for deterministic message handling
pub struct SequenceManager {
    /// Per-channel sequence state
    channels: Mutex<HashMap<String, SequenceState>>,
    /// Configuration
    config: SequenceConfig,
    /// Gap detection callback
    on_gap: Mutex<Option<GapCallback>>,
    /// Resync callback
    on_resync: Mutex<Option<ResyncCallback>>,
}

impl SequenceManager {
    /// Create new sequence manager with default config
    pub fn new() -> Self {
        Self::with_config(SequenceConfig::default())
    }
    
    /// Create sequence manager with custom config
    pub fn with_config(config: SequenceConfig) -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
            config,
            on_gap: Mutex::new(None),
            on_resync: Mutex::new(None),
        }
    }
    
    /// Set gap detection callback
    pub fn on_gap<F>(&self, callback: F) -> &Self
    where
        F: Fn(GapEvent) + Send + Sync + 'static
    {
        *self.on_gap.lock().unwrap() = Some(Arc::new(callback));
        self
    }
    
    /// Set resync callback
    pub fn on_resync<F>(&self, callback: F) -> &Self
    where
        F: Fn(ResyncEvent) + Send + Sync + 'static
    {
        *self.on_resync.lock().unwrap() = Some(Arc::new(callback));
        self
    }
    
    /// Validate and process a message with sequence number
    /// 
    /// Returns SequenceResult indicating whether message is in order
    /// and current state.
    pub fn validate(&self, channel: &str, sequence: u64, data: &str) -> SequenceResult {
        let mut channels = self.channels.lock().unwrap();
        let state = channels.entry(channel.to_string()).or_default();
        
        let expected = state.last_sequence + 1;
        
        // First message or in-order message
        if state.last_sequence == 0 || sequence == expected {
            state.last_sequence = sequence;
            state.messages_processed += 1;
            state.last_update = Utc::now();
            state.gap_detected = false;
            
            // Process any pending messages that are now in order
            self.process_pending(state);
            
            return SequenceResult {
                in_order: true,
                sequence,
                expected,
                gap_size: 0,
                resync_triggered: false,
                state: state.clone(),
            };
        }
        
        // Out of order - future message (gap detected)
        if sequence > expected {
            let gap_size = sequence - expected;
            state.gap_detected = true;
            state.total_gaps += 1;
            
            // Emit gap event
            self.emit_gap(GapEvent {
                channel: channel.to_string(),
                expected_sequence: expected,
                received_sequence: sequence,
                gap_size,
                timestamp: Utc::now(),
            });
            
            // Check if gap is too large
            if gap_size > self.config.max_gap_size && self.config.auto_resync {
                state.resync_triggered = true;
                self.trigger_resync(channel, state, ResyncReason::GapTooLarge { gap_size });
                
                return SequenceResult {
                    in_order: false,
                    sequence,
                    expected,
                    gap_size,
                    resync_triggered: true,
                    state: state.clone(),
                };
            }
            
            // Store as pending
            state.pending_messages.insert(sequence, PendingMessage {
                sequence,
                data: data.to_string(),
                received_at: Utc::now(),
            });
            
            // Check if too many pending
            if state.pending_messages.len() > self.config.max_pending_messages && self.config.auto_resync {
                state.resync_triggered = true;
                self.trigger_resync(channel, state, ResyncReason::TooManyPending { 
                    count: state.pending_messages.len() 
                });
                
                return SequenceResult {
                    in_order: false,
                    sequence,
                    expected,
                    gap_size,
                    resync_triggered: true,
                    state: state.clone(),
                };
            }
            
            return SequenceResult {
                in_order: false,
                sequence,
                expected,
                gap_size,
                resync_triggered: false,
                state: state.clone(),
            };
        }
        
        // Duplicate or old message (sequence < expected)
        SequenceResult {
            in_order: false,
            sequence,
            expected,
            gap_size: 0,
            resync_triggered: false,
            state: state.clone(),
        }
    }
    
    /// Process pending messages that are now in order
    fn process_pending(&self, state: &mut SequenceState) {
        loop {
            let next_expected = state.last_sequence + 1;
            if let Some(pending) = state.pending_messages.remove(&next_expected) {
                state.last_sequence = pending.sequence;
                state.messages_processed += 1;
                state.last_update = Utc::now();
            } else {
                break;
            }
        }
    }
    
    /// Trigger a resync
    fn trigger_resync(&self, channel: &str, state: &mut SequenceState, reason: ResyncReason) {
        let event = ResyncEvent {
            channel: channel.to_string(),
            last_good_sequence: state.last_sequence,
            reason,
            timestamp: Utc::now(),
        };
        
        // Clear pending messages
        state.pending_messages.clear();
        state.resync_triggered = true;
        
        // Emit resync event
        self.emit_resync(event);
    }
    
    /// Emit gap event to callback
    fn emit_gap(&self, event: GapEvent) {
        if let Some(callback) = self.on_gap.lock().unwrap().as_ref() {
            callback(event);
        }
    }
    
    /// Emit resync event to callback
    fn emit_resync(&self, event: ResyncEvent) {
        if let Some(callback) = self.on_resync.lock().unwrap().as_ref() {
            callback(event);
        }
    }
    
    /// Get current state for a channel
    pub fn get_state(&self, channel: &str) -> Option<SequenceState> {
        self.channels.lock().unwrap().get(channel).cloned()
    }
    
    /// Get all channel states
    pub fn get_all_states(&self) -> HashMap<String, SequenceState> {
        self.channels.lock().unwrap().clone()
    }
    
    /// Reset state for a channel
    pub fn reset(&self, channel: &str) {
        self.channels.lock().unwrap().remove(channel);
    }
    
    /// Reset all channels
    pub fn reset_all(&self) {
        self.channels.lock().unwrap().clear();
    }
    
    /// Manual resync request
    pub fn request_resync(&self, channel: &str) {
        let mut channels = self.channels.lock().unwrap();
        if let Some(state) = channels.get_mut(channel) {
            self.trigger_resync(channel, state, ResyncReason::ManualRequest);
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> SequenceStats {
        let channels = self.channels.lock().unwrap();
        let mut total_messages = 0u64;
        let mut total_gaps = 0u64;
        let mut channels_with_gaps = 0usize;
        
        for state in channels.values() {
            total_messages += state.messages_processed;
            total_gaps += state.total_gaps;
            if state.total_gaps > 0 {
                channels_with_gaps += 1;
            }
        }
        
        SequenceStats {
            total_channels: channels.len(),
            total_messages,
            total_gaps,
            channels_with_gaps,
            gap_rate: if total_messages > 0 {
                total_gaps as f64 / total_messages as f64
            } else {
                0.0
            },
        }
    }
}

impl Default for SequenceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Sequence statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceStats {
    pub total_channels: usize,
    pub total_messages: u64,
    pub total_gaps: u64,
    pub channels_with_gaps: usize,
    pub gap_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_in_order_messages() {
        let manager = SequenceManager::new();
        
        let r1 = manager.validate("test", 1, "msg1");
        assert!(r1.in_order);
        assert_eq!(r1.state.last_sequence, 1);
        
        let r2 = manager.validate("test", 2, "msg2");
        assert!(r2.in_order);
        assert_eq!(r2.state.last_sequence, 2);
        
        let r3 = manager.validate("test", 3, "msg3");
        assert!(r3.in_order);
        assert_eq!(r3.state.last_sequence, 3);
    }
    
    #[test]
    fn test_gap_detection() {
        let manager = SequenceManager::new();
        
        let r1 = manager.validate("test", 1, "msg1");
        assert!(r1.in_order);
        
        // Skip sequence 2, send 3
        let r3 = manager.validate("test", 3, "msg3");
        assert!(!r3.in_order);
        assert!(r3.state.gap_detected);
        assert_eq!(r3.gap_size, 1);
        assert_eq!(r3.expected, 2);
    }
    
    #[test]
    fn test_gap_recovery() {
        let manager = SequenceManager::new();
        
        manager.validate("test", 1, "msg1");
        manager.validate("test", 3, "msg3"); // Gap
        
        // Fill the gap
        let r2 = manager.validate("test", 2, "msg2");
        assert!(r2.in_order);
        
        // Now sequence 3 should be processed from pending
        let state = manager.get_state("test").unwrap();
        assert_eq!(state.last_sequence, 3);
    }
    
    #[test]
    fn test_resync_on_large_gap() {
        let config = SequenceConfig {
            max_gap_size: 5,
            auto_resync: true,
            ..Default::default()
        };
        let manager = SequenceManager::with_config(config);
        
        manager.validate("test", 1, "msg1");
        
        // Large gap triggers resync
        let result = manager.validate("test", 100, "msg100");
        assert!(!result.in_order);
        assert!(result.resync_triggered);
    }
    
    #[test]
    fn test_multiple_channels() {
        let manager = SequenceManager::new();
        
        manager.validate("btc", 1, "btc1");
        manager.validate("eth", 1, "eth1");
        manager.validate("btc", 2, "btc2");
        manager.validate("eth", 2, "eth2");
        
        let btc_state = manager.get_state("btc").unwrap();
        let eth_state = manager.get_state("eth").unwrap();
        
        assert_eq!(btc_state.last_sequence, 2);
        assert_eq!(eth_state.last_sequence, 2);
    }
}