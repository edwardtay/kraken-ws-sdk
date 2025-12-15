//! Event system and callback management

use crate::{data::*, error::SdkError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trait for event callbacks
pub trait EventCallback: Send + Sync {
    fn on_ticker(&self, data: TickerData);
    fn on_orderbook(&self, data: OrderBookUpdate);
    fn on_trade(&self, data: TradeData);
    fn on_ohlc(&self, data: OHLCData);
    fn on_error(&self, error: SdkError);
    fn on_connection_state_change(&self, state: ConnectionState);
}

/// Event dispatcher for managing callbacks
pub struct EventDispatcher {
    subscribers: Arc<Mutex<HashMap<DataType, Vec<CallbackEntry>>>>,
    connection_listeners: Arc<Mutex<Vec<CallbackEntry>>>,
    error_callbacks: Arc<Mutex<Vec<CallbackEntry>>>,
    next_id: Arc<Mutex<u64>>,
}

/// Callback entry with unique ID for management
#[derive(Clone)]
struct CallbackEntry {
    id: u64,
    callback: Arc<dyn EventCallback>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            connection_listeners: Arc::new(Mutex::new(Vec::new())),
            error_callbacks: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(0)),
        }
    }
    
    /// Generate next unique callback ID
    fn next_callback_id(&self) -> u64 {
        let mut id = self.next_id.lock().unwrap();
        *id += 1;
        *id
    }
    
    /// Register a callback for a specific data type
    pub fn register_callback(&self, data_type: DataType, callback: Arc<dyn EventCallback>) -> u64 {
        let id = self.next_callback_id();
        let entry = CallbackEntry { id, callback };
        
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.entry(data_type.clone()).or_insert_with(Vec::new).push(entry);
        
        tracing::debug!("Registered callback {} for data type {:?}", id, data_type);
        id
    }
    
    /// Register a callback for connection state changes
    pub fn register_connection_listener(&self, callback: Arc<dyn EventCallback>) -> u64 {
        let id = self.next_callback_id();
        let entry = CallbackEntry { id, callback };
        
        let mut listeners = self.connection_listeners.lock().unwrap();
        listeners.push(entry);
        
        tracing::debug!("Registered connection listener {}", id);
        id
    }
    
    /// Unregister a callback by ID
    pub fn unregister_callback(&self, data_type: DataType, callback_id: u64) -> bool {
        let mut subscribers = self.subscribers.lock().unwrap();
        if let Some(callbacks) = subscribers.get_mut(&data_type) {
            let initial_len = callbacks.len();
            callbacks.retain(|entry| entry.id != callback_id);
            let removed = callbacks.len() < initial_len;
            
            if removed {
                tracing::debug!("Unregistered callback {} for data type {:?}", callback_id, data_type);
            }
            
            // Remove empty entries
            if callbacks.is_empty() {
                subscribers.remove(&data_type);
            }
            
            removed
        } else {
            false
        }
    }
    
    /// Unregister a connection listener by ID
    pub fn unregister_connection_listener(&self, callback_id: u64) -> bool {
        let mut listeners = self.connection_listeners.lock().unwrap();
        let initial_len = listeners.len();
        listeners.retain(|entry| entry.id != callback_id);
        let removed = listeners.len() < initial_len;
        
        if removed {
            tracing::debug!("Unregistered connection listener {}", callback_id);
        }
        
        removed
    }
    
    /// Get count of registered callbacks for a data type
    pub fn get_callback_count(&self, data_type: &DataType) -> usize {
        let subscribers = self.subscribers.lock().unwrap();
        subscribers.get(data_type).map(|v| v.len()).unwrap_or(0)
    }
    
    /// Get count of connection listeners
    pub fn get_connection_listener_count(&self) -> usize {
        let listeners = self.connection_listeners.lock().unwrap();
        listeners.len()
    }
    
    /// Register a callback specifically for error handling
    pub fn register_error_callback(&self, callback: Arc<dyn EventCallback>) -> u64 {
        let id = self.next_callback_id();
        let entry = CallbackEntry { id, callback };
        
        let mut error_callbacks = self.error_callbacks.lock().unwrap();
        error_callbacks.push(entry);
        
        tracing::debug!("Registered error callback {}", id);
        id
    }
    
    /// Unregister an error callback by ID
    pub fn unregister_error_callback(&self, callback_id: u64) -> bool {
        let mut error_callbacks = self.error_callbacks.lock().unwrap();
        let initial_len = error_callbacks.len();
        error_callbacks.retain(|entry| entry.id != callback_id);
        let removed = error_callbacks.len() < initial_len;
        
        if removed {
            tracing::debug!("Unregistered error callback {}", callback_id);
        }
        
        removed
    }
    
    /// Notify error callbacks about processing failures
    fn notify_error_callbacks(&self, error: SdkError) {
        if let Ok(error_callbacks) = self.error_callbacks.lock() {
            for entry in error_callbacks.iter() {
                // Don't use panic catching here to avoid infinite recursion
                entry.callback.on_error(error.clone());
            }
        }
    }
    
    /// Dispatch ticker data to registered callbacks
    pub fn dispatch_ticker(&self, data: TickerData) {
        if let Ok(subscribers) = self.subscribers.lock() {
            if let Some(callbacks) = subscribers.get(&DataType::Ticker) {
                tracing::debug!("Dispatching ticker data to {} callbacks", callbacks.len());
                
                for (index, entry) in callbacks.iter().enumerate() {
                    // Handle callback errors gracefully
                    if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        entry.callback.on_ticker(data.clone());
                    })) {
                        let error_msg = format!("Callback {} (index {}) panicked while processing ticker data", entry.id, index);
                        tracing::error!("{}", error_msg);
                        
                        // Notify error callbacks about the callback failure
                        self.notify_error_callbacks(SdkError::Network(error_msg));
                    }
                }
            }
        }
    }
    
    /// Dispatch order book data to registered callbacks
    pub fn dispatch_orderbook(&self, data: OrderBookUpdate) {
        if let Ok(subscribers) = self.subscribers.lock() {
            if let Some(callbacks) = subscribers.get(&DataType::OrderBook) {
                tracing::debug!("Dispatching orderbook data to {} callbacks", callbacks.len());
                
                for (index, entry) in callbacks.iter().enumerate() {
                    if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        entry.callback.on_orderbook(data.clone());
                    })) {
                        let error_msg = format!("Callback {} (index {}) panicked while processing orderbook data", entry.id, index);
                        tracing::error!("{}", error_msg);
                        
                        // Notify error callbacks about the callback failure
                        self.notify_error_callbacks(SdkError::Network(error_msg));
                    }
                }
            }
        }
    }
    
    /// Dispatch trade data to registered callbacks
    pub fn dispatch_trade(&self, data: TradeData) {
        if let Ok(subscribers) = self.subscribers.lock() {
            if let Some(callbacks) = subscribers.get(&DataType::Trade) {
                tracing::debug!("Dispatching trade data to {} callbacks", callbacks.len());
                
                for (index, entry) in callbacks.iter().enumerate() {
                    if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        entry.callback.on_trade(data.clone());
                    })) {
                        let error_msg = format!("Callback {} (index {}) panicked while processing trade data", entry.id, index);
                        tracing::error!("{}", error_msg);
                        
                        // Notify error callbacks about the callback failure
                        self.notify_error_callbacks(SdkError::Network(error_msg));
                    }
                }
            }
        }
    }
    
    /// Dispatch OHLC data to registered callbacks
    pub fn dispatch_ohlc(&self, data: OHLCData) {
        if let Ok(subscribers) = self.subscribers.lock() {
            if let Some(callbacks) = subscribers.get(&DataType::OHLC) {
                tracing::debug!("Dispatching OHLC data to {} callbacks", callbacks.len());
                
                for (index, entry) in callbacks.iter().enumerate() {
                    if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        entry.callback.on_ohlc(data.clone());
                    })) {
                        let error_msg = format!("Callback {} (index {}) panicked while processing OHLC data", entry.id, index);
                        tracing::error!("{}", error_msg);
                        
                        // Notify error callbacks about the callback failure
                        self.notify_error_callbacks(SdkError::Network(error_msg));
                    }
                }
            }
        }
    }
    
    /// Dispatch error to all registered callbacks
    pub fn dispatch_error(&self, error: SdkError) {
        if let Ok(subscribers) = self.subscribers.lock() {
            let total_callbacks: usize = subscribers.values().map(|v| v.len()).sum();
            tracing::debug!("Dispatching error to {} callbacks", total_callbacks);
            
            for callbacks in subscribers.values() {
                for entry in callbacks {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        entry.callback.on_error(error.clone());
                    })).unwrap_or_else(|_| {
                        tracing::error!("Callback {} panicked while processing error", entry.id);
                    });
                }
            }
        }
    }
    
    /// Dispatch connection state change to registered listeners
    pub fn dispatch_connection_state_change(&self, state: ConnectionState) {
        if let Ok(listeners) = self.connection_listeners.lock() {
            tracing::debug!("Dispatching connection state change to {} listeners", listeners.len());
            
            for entry in listeners.iter() {
                if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    entry.callback.on_connection_state_change(state.clone());
                })) {
                    let error_msg = format!("Connection listener {} panicked while processing state change", entry.id);
                    tracing::error!("{}", error_msg);
                    
                    // Notify error callbacks about the callback failure
                    self.notify_error_callbacks(SdkError::Network(error_msg));
                }
            }
        }
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventDispatcher {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
            connection_listeners: Arc::clone(&self.connection_listeners),
            error_callbacks: Arc::clone(&self.error_callbacks),
            next_id: Arc::clone(&self.next_id),
        }
    }
}