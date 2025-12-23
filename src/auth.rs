//! Authentication utilities for Kraken API
//!
//! Implements HMAC-SHA512 request signing as per Kraken API docs:
//! https://docs.kraken.com/rest/#section/Authentication

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Digest, Sha256, Sha512};
use hmac::{Hmac, Mac};
use crate::error::SdkError;

type HmacSha512 = Hmac<Sha512>;

/// Credentials for authenticated API access
#[derive(Clone)]
pub struct Credentials {
    api_key: String,
    api_secret: Vec<u8>,
}

impl Credentials {
    /// Create new credentials from API key and secret
    ///
    /// # Arguments
    /// * `api_key` - Kraken API key
    /// * `api_secret` - Base64-encoded API secret from Kraken
    ///
    /// # Example
    /// ```rust,ignore
    /// use kraken_ws_sdk::auth::Credentials;
    ///
    /// let creds = Credentials::new(
    ///     "your-api-key",
    ///     "your-base64-secret"
    /// )?;
    /// ```
    pub fn new(api_key: &str, api_secret: &str) -> Result<Self, SdkError> {
        let decoded_secret = BASE64
            .decode(api_secret)
            .map_err(|e| SdkError::Authentication(format!("Invalid API secret encoding: {}", e)))?;

        Ok(Self {
            api_key: api_key.to_string(),
            api_secret: decoded_secret,
        })
    }

    /// Create credentials from environment variables
    ///
    /// Looks for `KRAKEN_API_KEY` and `KRAKEN_API_SECRET`
    pub fn from_env() -> Result<Self, SdkError> {
        let api_key = std::env::var("KRAKEN_API_KEY")
            .map_err(|_| SdkError::Authentication("KRAKEN_API_KEY not set".to_string()))?;
        let api_secret = std::env::var("KRAKEN_API_SECRET")
            .map_err(|_| SdkError::Authentication("KRAKEN_API_SECRET not set".to_string()))?;

        Self::new(&api_key, &api_secret)
    }

    /// Get the API key (for request headers)
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Sign a REST API request
    ///
    /// Kraken signature = HMAC-SHA512(path + SHA256(nonce + postdata), secret)
    ///
    /// # Arguments
    /// * `uri_path` - API endpoint path (e.g., "/0/private/Balance")
    /// * `nonce` - Unique incrementing integer
    /// * `post_data` - URL-encoded POST body
    ///
    /// # Returns
    /// Base64-encoded signature for API-Sign header
    pub fn sign(&self, uri_path: &str, nonce: u64, post_data: &str) -> Result<String, SdkError> {
        // Step 1: SHA256(nonce + postdata)
        let nonce_str = nonce.to_string();
        let mut sha256 = Sha256::new();
        sha256.update(nonce_str.as_bytes());
        sha256.update(post_data.as_bytes());
        let sha256_hash = sha256.finalize();

        // Step 2: Concatenate path + sha256_hash
        let mut message = uri_path.as_bytes().to_vec();
        message.extend_from_slice(&sha256_hash);

        // Step 3: HMAC-SHA512(message, secret)
        let mut mac = HmacSha512::new_from_slice(&self.api_secret)
            .map_err(|e| SdkError::Authentication(format!("HMAC error: {}", e)))?;
        mac.update(&message);
        let signature = mac.finalize().into_bytes();

        // Step 4: Base64 encode
        Ok(BASE64.encode(signature))
    }

    /// Generate a nonce (microseconds since epoch)
    pub fn generate_nonce() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_micros() as u64
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("api_key", &format!("{}...", &self.api_key.chars().take(8).collect::<String>()))
            .field("api_secret", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_creation() {
        // Test with a valid base64 secret
        let secret = BASE64.encode(b"test_secret_key_here");
        let creds = Credentials::new("test_key", &secret);
        assert!(creds.is_ok());
    }

    #[test]
    fn test_invalid_secret_encoding() {
        let creds = Credentials::new("test_key", "not-valid-base64!!!");
        assert!(creds.is_err());
    }

    #[test]
    fn test_nonce_generation() {
        let nonce1 = Credentials::generate_nonce();
        std::thread::sleep(std::time::Duration::from_micros(10));
        let nonce2 = Credentials::generate_nonce();
        assert!(nonce2 > nonce1);
    }

    #[test]
    fn test_signature_deterministic() {
        let secret = BASE64.encode(b"test_secret");
        let creds = Credentials::new("key", &secret).unwrap();
        
        let sig1 = creds.sign("/0/private/Balance", 1234567890, "nonce=1234567890").unwrap();
        let sig2 = creds.sign("/0/private/Balance", 1234567890, "nonce=1234567890").unwrap();
        
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_debug_redacts_secret() {
        let secret = BASE64.encode(b"super_secret");
        let creds = Credentials::new("my_api_key_12345", &secret).unwrap();
        let debug_str = format!("{:?}", creds);
        
        assert!(debug_str.contains("my_api_k..."));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("super_secret"));
    }
}
