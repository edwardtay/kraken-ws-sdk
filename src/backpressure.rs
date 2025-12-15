//! Backpressure and Throttling Control
//! 
//! Production-grade flow control for high-frequency trading infrastructure.
//! Handles message rate limiting, drop policies, and update coalescing.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Drop policy when buffer is full or rate exceeded
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropPolicy {
    /// Drop oldest messages first (FIFO overflow)
    Oldest,
    /// Drop newest messages (reject new)
    Latest,
    /// Drop randomly to maintain statistical fairness
    Random,
    /// Never drop, block instead (careful with this!)
    Block,
}

impl Default for DropPolicy {
    fn default() -> Self {
        DropPolicy::Oldest
    }
}

/// Backpressure configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpressureConfig {
    /// Maximum messages per second (0 = unlimited)
    pub max_messages_per_second: u32,
    /// Maximum buffer size per channel
    pub max_buffer_size: usize,
    /// Drop policy when limits exceeded
    pub drop_policy: DropPolicy,
    /// Coalesce updates for same symbol (keep latest)
    pub coalesce_updates: bool,
    /// Burst allowance (messages above rate for short bursts)
    pub burst_allowance: u32,
    /// Window size for rate calculation (ms)
    pub rate_window_ms: u64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_messages_per_second: 1000,
            max_buffer_size: 10000,
            drop_policy: DropPolicy::Oldest,
            coalesce_updates: true,
            burst_allowance: 100,
            rate_window_ms: 1000,
        }
    }
}

/// Result of processing a message through backpressure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpressureResult {
    /// Whether message was accepted
    pub accepted: bool,
    /// Whether message was dropped
    pub dropped: bool,
    /// Whether message was coalesced with existing
    pub coalesced: bool,
    /// Current queue depth
    pub queue_depth: usize,
    /// Current rate (messages/sec)
    pub current_rate: f64,
    /// Messages dropped in this window
    pub dropped_count: u64,
    /// Messages coalesced in this window
    pub coalesced_count: u64,
}

/// Statistics for backpressure monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackpressureStats {
    /// Total messages received
    pub total_received: u64,
    /// Total messages accepted
    pub total_accepted: u64,
    /// Total messages dropped
    pub total_dropped: u64,
    /// Total messages coalesced
    pub total_coalesced: u64,
    /// Peak rate observed (msg/sec)
    pub peak_rate: f64,
    /// Current rate (msg/sec)
    pub current_rate: f64,
    /// Peak queue depth
    pub peak_queue_depth: usize,
    /// Current queue depth
    pub current_queue_depth: usize,
    /// Drop rate percentage
    pub drop_rate: f64,
    /// Coalesce rate percentage
    pub coalesce_rate: f64,
}

/// Message wrapper with metadata
#[derive(Debug, Clone)]
pub struct BufferedMessage {
    pub channel: String,
    pub symbol: String,
    pub data: String,
    pub sequence: Option<u64>,
    pub received_at: Instant,
    pub timestamp: DateTime<Utc>,
}

/// Per-channel state
#[derive(Debug)]
struct ChannelState {
    buffer: VecDeque<BufferedMessage>,
    /// Coalesced messages by symbol (latest only)
    coalesced: HashMap<String, BufferedMessage>,
    /// Timestamps of recent messages for rate calculation
    recent_timestamps: VecDeque<Instant>,
    /// Stats for this channel
    stats: BackpressureStats,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            buffer: VecDeque::new(),
            coalesced: HashMap::new(),
            recent_timestamps: VecDeque::new(),
            stats: BackpressureStats::default(),
        }
    }
}

/// Callback for drop events
pub type DropCallback = Arc<dyn Fn(DropEvent) + Send + Sync>;

/// Callback for coalesce events
pub type CoalesceCallback = Arc<dyn Fn(CoalesceEvent) + Send + Sync>;

/// Callback for rate limit events
pub type RateLimitCallback = Arc<dyn Fn(RateLimitEvent) + Send + Sync>;

/// Drop event details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropEvent {
    pub channel: String,
    pub symbol: String,
    pub reason: DropReason,
    pub dropped_count: u64,
    pub queue_depth: usize,
    pub current_rate: f64,
    pub timestamp: DateTime<Utc>,
}

/// Reason for dropping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DropReason {
    RateLimitExceeded { rate: f64, limit: u32 },
    BufferFull { size: usize, max: usize },
    PolicyDecision { policy: DropPolicy },
}

/// Coalesce event details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoalesceEvent {
    pub channel: String,
    pub symbol: String,
    pub old_sequence: Option<u64>,
    pub new_sequence: Option<u64>,
    pub coalesced_count: u64,
    pub timestamp: DateTime<Utc>,
}

/// Rate limit event details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEvent {
    pub channel: String,
    pub current_rate: f64,
    pub limit: u32,
    pub action_taken: String,
    pub timestamp: DateTime<Utc>,
}

/// Production-grade backpressure manager
pub struct BackpressureManager {
    config: BackpressureConfig,
    channels: Mutex<HashMap<String, ChannelState>>,
    global_stats: Mutex<BackpressureStats>,
    on_drop: Mutex<Option<DropCallback>>,
    on_coalesce: Mutex<Option<CoalesceCallback>>,
    on_rate_limit: Mutex<Option<RateLimitCallback>>,
}

impl BackpressureManager {
    /// Create with default config
    pub fn new() -> Self {
        Self::with_config(BackpressureConfig::default())
    }
    
    /// Create with custom config
    pub fn with_config(config: BackpressureConfig) -> Self {
        Self {
            config,
            channels: Mutex::new(HashMap::new()),
            global_stats: Mutex::new(BackpressureStats::default()),
            on_drop: Mutex::new(None),
            on_coalesce: Mutex::new(None),
            on_rate_limit: Mutex::new(None),
        }
    }
    
    /// Set drop callback
    pub fn on_drop<F>(&self, callback: F) -> &Self
    where
        F: Fn(DropEvent) + Send + Sync + 'static
    {
        *self.on_drop.lock().unwrap() = Some(Arc::new(callback));
        self
    }
    
    /// Set coalesce callback
    pub fn on_coalesce<F>(&self, callback: F) -> &Self
    where
        F: Fn(CoalesceEvent) + Send + Sync + 'static
    {
        *self.on_coalesce.lock().unwrap() = Some(Arc::new(callback));
        self
    }
    
    /// Set rate limit callback
    pub fn on_rate_limit<F>(&self, callback: F) -> &Self
    where
        F: Fn(RateLimitEvent) + Send + Sync + 'static
    {
        *self.on_rate_limit.lock().unwrap() = Some(Arc::new(callback));
        self
    }
    
    /// Process incoming message through backpressure control
    pub fn process(&self, message: BufferedMessage) -> BackpressureResult {
        let mut channels = self.channels.lock().unwrap();
        let state = channels.entry(message.channel.clone()).or_default();
        
        // Update global stats
        {
            let mut global = self.global_stats.lock().unwrap();
            global.total_received += 1;
        }
        state.stats.total_received += 1;
        
        let now = Instant::now();
        
        // Clean old timestamps outside rate window
        let window = Duration::from_millis(self.config.rate_window_ms);
        while let Some(ts) = state.recent_timestamps.front() {
            if now.duration_since(*ts) > window {
                state.recent_timestamps.pop_front();
            } else {
                break;
            }
        }
        
        // Calculate current rate
        let current_rate = state.recent_timestamps.len() as f64 
            / (self.config.rate_window_ms as f64 / 1000.0);
        state.stats.current_rate = current_rate;
        
        if current_rate > state.stats.peak_rate {
            state.stats.peak_rate = current_rate;
        }
        
        // Check rate limit
        let rate_limit = self.config.max_messages_per_second as f64;
        let burst_limit = rate_limit + self.config.burst_allowance as f64;
        
        if self.config.max_messages_per_second > 0 && current_rate >= burst_limit {
            // Rate limit exceeded
            self.emit_rate_limit(RateLimitEvent {
                channel: message.channel.clone(),
                current_rate,
                limit: self.config.max_messages_per_second,
                action_taken: format!("Applying {:?} policy", self.config.drop_policy),
                timestamp: Utc::now(),
            });
            
            match self.config.drop_policy {
                DropPolicy::Latest => {
                    // Drop this message
                    state.stats.total_dropped += 1;
                    self.emit_drop(DropEvent {
                        channel: message.channel.clone(),
                        symbol: message.symbol.clone(),
                        reason: DropReason::RateLimitExceeded { rate: current_rate, limit: self.config.max_messages_per_second },
                        dropped_count: state.stats.total_dropped,
                        queue_depth: state.buffer.len(),
                        current_rate,
                        timestamp: Utc::now(),
                    });
                    
                    return BackpressureResult {
                        accepted: false,
                        dropped: true,
                        coalesced: false,
                        queue_depth: state.buffer.len(),
                        current_rate,
                        dropped_count: state.stats.total_dropped,
                        coalesced_count: state.stats.total_coalesced,
                    };
                }
                DropPolicy::Oldest => {
                    // Drop oldest from buffer
                    if let Some(dropped) = state.buffer.pop_front() {
                        state.stats.total_dropped += 1;
                        self.emit_drop(DropEvent {
                            channel: dropped.channel,
                            symbol: dropped.symbol,
                            reason: DropReason::RateLimitExceeded { rate: current_rate, limit: self.config.max_messages_per_second },
                            dropped_count: state.stats.total_dropped,
                            queue_depth: state.buffer.len(),
                            current_rate,
                            timestamp: Utc::now(),
                        });
                    }
                }
                DropPolicy::Random => {
                    // Drop random message
                    if !state.buffer.is_empty() {
                        let idx = (now.elapsed().as_nanos() as usize) % state.buffer.len();
                        if let Some(dropped) = state.buffer.remove(idx) {
                            state.stats.total_dropped += 1;
                            self.emit_drop(DropEvent {
                                channel: dropped.channel,
                                symbol: dropped.symbol,
                                reason: DropReason::PolicyDecision { policy: DropPolicy::Random },
                                dropped_count: state.stats.total_dropped,
                                queue_depth: state.buffer.len(),
                                current_rate,
                                timestamp: Utc::now(),
                            });
                        }
                    }
                }
                DropPolicy::Block => {
                    // Block - just continue (caller should handle)
                }
            }
        }
        
        // Check buffer size
        if state.buffer.len() >= self.config.max_buffer_size {
            match self.config.drop_policy {
                DropPolicy::Latest => {
                    state.stats.total_dropped += 1;
                    self.emit_drop(DropEvent {
                        channel: message.channel.clone(),
                        symbol: message.symbol.clone(),
                        reason: DropReason::BufferFull { size: state.buffer.len(), max: self.config.max_buffer_size },
                        dropped_count: state.stats.total_dropped,
                        queue_depth: state.buffer.len(),
                        current_rate,
                        timestamp: Utc::now(),
                    });
                    
                    return BackpressureResult {
                        accepted: false,
                        dropped: true,
                        coalesced: false,
                        queue_depth: state.buffer.len(),
                        current_rate,
                        dropped_count: state.stats.total_dropped,
                        coalesced_count: state.stats.total_coalesced,
                    };
                }
                DropPolicy::Oldest => {
                    if let Some(dropped) = state.buffer.pop_front() {
                        state.stats.total_dropped += 1;
                        self.emit_drop(DropEvent {
                            channel: dropped.channel,
                            symbol: dropped.symbol,
                            reason: DropReason::BufferFull { size: state.buffer.len(), max: self.config.max_buffer_size },
                            dropped_count: state.stats.total_dropped,
                            queue_depth: state.buffer.len(),
                            current_rate,
                            timestamp: Utc::now(),
                        });
                    }
                }
                _ => {}
            }
        }
        
        // Handle coalescing
        let mut coalesced = false;
        if self.config.coalesce_updates {
            if let Some(existing) = state.coalesced.get(&message.symbol) {
                // Coalesce - replace with newer
                state.stats.total_coalesced += 1;
                coalesced = true;
                
                self.emit_coalesce(CoalesceEvent {
                    channel: message.channel.clone(),
                    symbol: message.symbol.clone(),
                    old_sequence: existing.sequence,
                    new_sequence: message.sequence,
                    coalesced_count: state.stats.total_coalesced,
                    timestamp: Utc::now(),
                });
            }
            state.coalesced.insert(message.symbol.clone(), message.clone());
        } else {
            // No coalescing, add to buffer
            state.buffer.push_back(message.clone());
        }
        
        // Record timestamp for rate calculation
        state.recent_timestamps.push_back(now);
        
        // Update stats
        state.stats.total_accepted += 1;
        state.stats.current_queue_depth = state.buffer.len() + state.coalesced.len();
        if state.stats.current_queue_depth > state.stats.peak_queue_depth {
            state.stats.peak_queue_depth = state.stats.current_queue_depth;
        }
        
        // Calculate rates
        if state.stats.total_received > 0 {
            state.stats.drop_rate = state.stats.total_dropped as f64 / state.stats.total_received as f64 * 100.0;
            state.stats.coalesce_rate = state.stats.total_coalesced as f64 / state.stats.total_received as f64 * 100.0;
        }
        
        // Update global stats
        {
            let mut global = self.global_stats.lock().unwrap();
            global.total_accepted += 1;
            if coalesced {
                global.total_coalesced += 1;
            }
        }
        
        BackpressureResult {
            accepted: true,
            dropped: false,
            coalesced,
            queue_depth: state.stats.current_queue_depth,
            current_rate,
            dropped_count: state.stats.total_dropped,
            coalesced_count: state.stats.total_coalesced,
        }
    }
    
    /// Get next message from buffer (for processing)
    pub fn pop(&self, channel: &str) -> Option<BufferedMessage> {
        let mut channels = self.channels.lock().unwrap();
        if let Some(state) = channels.get_mut(channel) {
            // If coalescing, drain coalesced first
            if self.config.coalesce_updates && !state.coalesced.is_empty() {
                let key = state.coalesced.keys().next().cloned();
                if let Some(k) = key {
                    return state.coalesced.remove(&k);
                }
            }
            return state.buffer.pop_front();
        }
        None
    }
    
    /// Get stats for a channel
    pub fn get_stats(&self, channel: &str) -> Option<BackpressureStats> {
        self.channels.lock().unwrap().get(channel).map(|s| s.stats.clone())
    }
    
    /// Get global stats
    pub fn global_stats(&self) -> BackpressureStats {
        let mut stats = self.global_stats.lock().unwrap().clone();
        
        // Aggregate from all channels
        let channels = self.channels.lock().unwrap();
        for state in channels.values() {
            stats.total_dropped += state.stats.total_dropped;
            if state.stats.peak_rate > stats.peak_rate {
                stats.peak_rate = state.stats.peak_rate;
            }
            stats.current_queue_depth += state.stats.current_queue_depth;
        }
        
        if stats.total_received > 0 {
            stats.drop_rate = stats.total_dropped as f64 / stats.total_received as f64 * 100.0;
            stats.coalesce_rate = stats.total_coalesced as f64 / stats.total_received as f64 * 100.0;
        }
        
        stats
    }
    
    /// Get current config
    pub fn config(&self) -> &BackpressureConfig {
        &self.config
    }
    
    /// Reset stats
    pub fn reset_stats(&self) {
        *self.global_stats.lock().unwrap() = BackpressureStats::default();
        for state in self.channels.lock().unwrap().values_mut() {
            state.stats = BackpressureStats::default();
        }
    }
    
    /// Clear all buffers
    pub fn clear(&self) {
        for state in self.channels.lock().unwrap().values_mut() {
            state.buffer.clear();
            state.coalesced.clear();
        }
    }
    
    fn emit_drop(&self, event: DropEvent) {
        if let Some(cb) = self.on_drop.lock().unwrap().as_ref() {
            cb(event);
        }
    }
    
    fn emit_coalesce(&self, event: CoalesceEvent) {
        if let Some(cb) = self.on_coalesce.lock().unwrap().as_ref() {
            cb(event);
        }
    }
    
    fn emit_rate_limit(&self, event: RateLimitEvent) {
        if let Some(cb) = self.on_rate_limit.lock().unwrap().as_ref() {
            cb(event);
        }
    }
}

impl Default for BackpressureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_message(channel: &str, symbol: &str) -> BufferedMessage {
        BufferedMessage {
            channel: channel.to_string(),
            symbol: symbol.to_string(),
            data: "test".to_string(),
            sequence: Some(1),
            received_at: Instant::now(),
            timestamp: Utc::now(),
        }
    }
    
    #[test]
    fn test_basic_accept() {
        let manager = BackpressureManager::new();
        let msg = make_message("ticker", "BTC/USD");
        let result = manager.process(msg);
        
        assert!(result.accepted);
        assert!(!result.dropped);
    }
    
    #[test]
    fn test_coalescing() {
        let config = BackpressureConfig {
            coalesce_updates: true,
            ..Default::default()
        };
        let manager = BackpressureManager::with_config(config);
        
        // Send two messages for same symbol
        let msg1 = make_message("ticker", "BTC/USD");
        let msg2 = make_message("ticker", "BTC/USD");
        
        let r1 = manager.process(msg1);
        let r2 = manager.process(msg2);
        
        assert!(r1.accepted);
        assert!(r2.accepted);
        assert!(r2.coalesced);
    }
    
    #[test]
    fn test_buffer_limit() {
        let config = BackpressureConfig {
            max_buffer_size: 5,
            coalesce_updates: false,
            drop_policy: DropPolicy::Latest,
            ..Default::default()
        };
        let manager = BackpressureManager::with_config(config);
        
        // Fill buffer
        for i in 0..5 {
            let msg = BufferedMessage {
                channel: "ticker".to_string(),
                symbol: format!("SYM{}", i),
                data: "test".to_string(),
                sequence: Some(i as u64),
                received_at: Instant::now(),
                timestamp: Utc::now(),
            };
            manager.process(msg);
        }
        
        // Next message should be dropped
        let msg = make_message("ticker", "OVERFLOW");
        let result = manager.process(msg);
        
        assert!(!result.accepted);
        assert!(result.dropped);
    }
    
    #[test]
    fn test_drop_policy_oldest() {
        let config = BackpressureConfig {
            max_buffer_size: 3,
            coalesce_updates: false,
            drop_policy: DropPolicy::Oldest,
            ..Default::default()
        };
        let manager = BackpressureManager::with_config(config);
        
        // Fill buffer
        for i in 0..3 {
            let msg = BufferedMessage {
                channel: "ticker".to_string(),
                symbol: format!("SYM{}", i),
                data: "test".to_string(),
                sequence: Some(i as u64),
                received_at: Instant::now(),
                timestamp: Utc::now(),
            };
            manager.process(msg);
        }
        
        // Add one more - oldest should be dropped
        let msg = make_message("ticker", "NEW");
        let result = manager.process(msg);
        
        assert!(result.accepted);
        assert_eq!(result.dropped_count, 1);
    }
    
    #[test]
    fn test_stats() {
        let manager = BackpressureManager::new();
        
        for _ in 0..10 {
            let msg = make_message("ticker", "BTC/USD");
            manager.process(msg);
        }
        
        let stats = manager.global_stats();
        assert_eq!(stats.total_received, 10);
        assert_eq!(stats.total_accepted, 10);
    }
}