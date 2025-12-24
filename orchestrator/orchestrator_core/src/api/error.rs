//! API error types and responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use orchestrator_shared_types::OrchestrationError;

/// API error response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    /// Human-readable error message.
    pub error: String,
    /// Machine-readable error code.
    pub code: String,
    /// Optional details about the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    // Common error constructors
    pub fn not_found(resource: &str, id: &str) -> Self {
        Self::new(
            format!("{} not found: {}", resource, id),
            "NOT_FOUND",
        )
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(message, "BAD_REQUEST")
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(message, "INTERNAL_ERROR")
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(message, "CONFLICT")
    }

    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::new(message, "VALIDATION_ERROR")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.code.as_str() {
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            "BAD_REQUEST" | "VALIDATION_ERROR" => StatusCode::BAD_REQUEST,
            "CONFLICT" => StatusCode::CONFLICT,
            "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
            "FORBIDDEN" => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}

impl From<OrchestrationError> for ApiError {
    fn from(err: OrchestrationError) -> Self {
        match err {
            OrchestrationError::NodeNotFound(id) => {
                ApiError::not_found("Node", &id.to_string())
            }
            OrchestrationError::WorkloadNotFound(id) => {
                ApiError::not_found("Workload", &id.to_string())
            }
            OrchestrationError::RuntimeError(msg) => {
                ApiError::internal_error(format!("Runtime error: {}", msg))
            }
            OrchestrationError::SchedulingError(msg) => {
                ApiError::internal_error(format!("Scheduling error: {}", msg))
            }
            OrchestrationError::ClusterError(msg) => {
                ApiError::internal_error(format!("Cluster error: {}", msg))
            }
            OrchestrationError::StateError(msg) => {
                ApiError::internal_error(format!("State error: {}", msg))
            }
            OrchestrationError::ConfigError(msg) => {
                ApiError::bad_request(format!("Configuration error: {}", msg))
            }
            OrchestrationError::NetworkError(msg) => {
                ApiError::internal_error(format!("Network error: {}", msg))
            }
            OrchestrationError::InternalError(msg) => {
                ApiError::internal_error(msg)
            }
            OrchestrationError::NotImplemented(msg) => {
                ApiError::new(msg, "NOT_IMPLEMENTED")
            }
        }
    }
}

/// Result type for API handlers.
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_creation() {
        let error = ApiError::not_found("Workload", "123");
        assert!(error.error.contains("Workload"));
        assert!(error.error.contains("123"));
        assert_eq!(error.code, "NOT_FOUND");
    }

    #[test]
    fn test_api_error_with_details() {
        let error = ApiError::bad_request("Invalid input")
            .with_details(serde_json::json!({"field": "name", "reason": "too short"}));

        assert!(error.details.is_some());
    }

    #[test]
    fn test_orchestration_error_conversion() {
        let orch_err = OrchestrationError::NodeNotFound(uuid::Uuid::new_v4());
        let api_err: ApiError = orch_err.into();
        assert_eq!(api_err.code, "NOT_FOUND");
    }
}
