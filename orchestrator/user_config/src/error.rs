//! Error types for the user_config crate.

use thiserror::Error;

/// Result type for user_config operations.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Errors that can occur in user configuration operations.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// I/O error when reading or writing configuration files.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Error parsing TOML configuration.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Error serializing to TOML.
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    /// Error with JSON serialization.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Cryptographic operation failed.
    #[error("Cryptographic error: {0}")]
    Crypto(String),

    /// Age encryption/decryption error.
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Invalid key format or data.
    #[error("Invalid key: {0}")]
    InvalidKey(String),

    /// Configuration file not found.
    #[error("Configuration not found: {0}")]
    NotFound(String),

    /// Configuration validation failed.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Permission denied for operation.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Trust policy violation.
    #[error("Trust policy violation: {0}")]
    TrustViolation(String),

    /// Resource limit exceeded.
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Automation boundary violation.
    #[error("Automation boundary violation: {0}")]
    AutomationBoundary(String),

    /// Configuration path error.
    #[error("Path error: {0}")]
    PathError(String),

    /// Base64 decoding error.
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// Version mismatch.
    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
}

impl ConfigError {
    /// Create a new crypto error.
    pub fn crypto(msg: impl Into<String>) -> Self {
        Self::Crypto(msg.into())
    }

    /// Create a new encryption error.
    pub fn encryption(msg: impl Into<String>) -> Self {
        Self::Encryption(msg.into())
    }

    /// Create a new validation error.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }
}
