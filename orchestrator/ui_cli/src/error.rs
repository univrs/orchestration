//! CLI error types.

#![allow(dead_code)]

use thiserror::Error;

/// CLI error type.
#[derive(Error, Debug)]
pub enum CliError {
    #[error("API request failed: {0}")]
    ApiError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Workload not found: {0}")]
    WorkloadNotFound(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("User config error: {0}")]
    UserConfigError(#[from] user_config::ConfigError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl CliError {
    pub fn api_error(msg: impl Into<String>) -> Self {
        Self::ApiError(msg.into())
    }

    pub fn config_error(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }

    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
