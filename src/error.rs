//! Error types for the Kraken WebSocket SDK

use thiserror::Error;
use std::fmt;
use chrono;
use std::collections;

/// Main error type for the SDK
#[derive(Error, Debug, Clone)]
pub enum SdkError {
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),
    
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    
    #[error("Subscription error: {0}")]
    Subscription(#[from] SubscriptionError),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Connection-specific errors
#[derive(Error, Debug, Clone)]
pub enum ConnectionError {
    #[error("Failed to establish connection: {0}")]
    EstablishmentFailed(String),
    
    #[error("Connection lost: {0}")]
    ConnectionLost(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Timeout occurred: {0}")]
    Timeout(String),
}

/// Parsing-specific errors
#[derive(Error, Debug, Clone)]
pub enum ParseError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    
    #[error("Missing field: {0}")]
    MissingField(String),
    
    #[error("Invalid data type: {0}")]
    InvalidDataType(String),
    
    #[error("Malformed message: {0}")]
    MalformedMessage(String),
}

/// Subscription-specific errors
#[derive(Error, Debug, Clone)]
pub enum SubscriptionError {
    #[error("Invalid channel: {0}")]
    InvalidChannel(String),
    
    #[error("Subscription failed: {0}")]
    SubscriptionFailed(String),
    
    #[error("Already subscribed: {0}")]
    AlreadySubscribed(String),
    
    #[error("Not subscribed: {0}")]
    NotSubscribed(String),
}

/// Processing-specific errors
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    #[error("Message processing failed: {0}")]
    ProcessingFailed(String),
    
    #[error("Callback error: {0}")]
    CallbackError(String),
}

/// Error context for debugging
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub operation: String,
    pub details: std::collections::HashMap<String, String>,
    pub stack_trace: Option<String>,
}

impl ErrorContext {
    pub fn new(operation: &str) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            operation: operation.to_string(),
            details: std::collections::HashMap::new(),
            stack_trace: None,
        }
    }
    
    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }
    
    pub fn with_stack_trace(mut self, trace: String) -> Self {
        self.stack_trace = Some(trace);
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] Operation: {}", self.timestamp, self.operation)?;
        
        if !self.details.is_empty() {
            write!(f, " | Details: {:?}", self.details)?;
        }
        
        if let Some(trace) = &self.stack_trace {
            write!(f, " | Stack: {}", trace)?;
        }
        
        Ok(())
    }
}

/// Enhanced error with context
#[derive(Debug, Clone)]
pub struct ContextualError {
    pub error: SdkError,
    pub context: ErrorContext,
}

impl ContextualError {
    pub fn new(error: SdkError, context: ErrorContext) -> Self {
        Self { error, context }
    }
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} | Context: {}", self.error, self.context)
    }
}

impl std::error::Error for ContextualError {}

/// Error severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Low,      // Warnings, non-critical issues
    Medium,   // Recoverable errors
    High,     // Critical errors that affect functionality
    Critical, // System-level failures
}

impl ErrorSeverity {
    pub fn from_error(error: &SdkError) -> Self {
        match error {
            SdkError::Configuration(_) => ErrorSeverity::High,
            SdkError::Authentication(_) => ErrorSeverity::High,
            SdkError::Connection(conn_err) => match conn_err {
                ConnectionError::AuthenticationFailed(_) => ErrorSeverity::Critical,
                ConnectionError::Timeout(_) => ErrorSeverity::Medium,
                _ => ErrorSeverity::High,
            },
            SdkError::Parse(_) => ErrorSeverity::Low,
            SdkError::Subscription(_) => ErrorSeverity::Medium,
            SdkError::Network(_) => ErrorSeverity::Medium,
            SdkError::NotImplemented(_) => ErrorSeverity::Low,
        }
    }
}

/// Error reporter for structured logging and monitoring
pub struct ErrorReporter;

impl ErrorReporter {
    pub fn report_error(error: &SdkError, context: Option<ErrorContext>) {
        let severity = ErrorSeverity::from_error(error);
        
        match severity {
            ErrorSeverity::Critical => {
                tracing::error!("CRITICAL ERROR: {} | Context: {:?}", error, context);
            }
            ErrorSeverity::High => {
                tracing::error!("HIGH SEVERITY: {} | Context: {:?}", error, context);
            }
            ErrorSeverity::Medium => {
                tracing::warn!("MEDIUM SEVERITY: {} | Context: {:?}", error, context);
            }
            ErrorSeverity::Low => {
                tracing::debug!("LOW SEVERITY: {} | Context: {:?}", error, context);
            }
        }
    }
    
    pub fn report_contextual_error(contextual_error: &ContextualError) {
        let severity = ErrorSeverity::from_error(&contextual_error.error);
        
        match severity {
            ErrorSeverity::Critical => {
                tracing::error!("CRITICAL: {}", contextual_error);
            }
            ErrorSeverity::High => {
                tracing::error!("HIGH: {}", contextual_error);
            }
            ErrorSeverity::Medium => {
                tracing::warn!("MEDIUM: {}", contextual_error);
            }
            ErrorSeverity::Low => {
                tracing::debug!("LOW: {}", contextual_error);
            }
        }
    }
}