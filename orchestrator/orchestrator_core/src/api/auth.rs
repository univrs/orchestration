//! Ed25519 request signing authentication middleware.
//!
//! Implements request authentication using Ed25519 signatures.
//!
//! # Headers
//!
//! Clients must include the following headers:
//! - `X-Auth-PublicKey`: Base64-encoded Ed25519 public key (32 bytes)
//! - `X-Auth-Timestamp`: ISO8601 timestamp (e.g., "2025-01-15T12:00:00Z")
//! - `X-Auth-Signature`: Base64-encoded Ed25519 signature (64 bytes)
//!
//! # Signing String
//!
//! The signature is computed over the following string:
//! ```text
//! METHOD\nPATH\nTIMESTAMP\nBODY_SHA256_HEX
//! ```
//!
//! Where BODY_SHA256_HEX is the lowercase hex SHA-256 hash of the request body.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header::HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::prelude::*;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Authentication error response.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthError {
    pub error: String,
    pub code: String,
}

impl AuthError {
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
        }
    }

    pub fn missing_header(header: &str) -> Self {
        Self::new(
            format!("Missing required header: {}", header),
            "MISSING_HEADER",
        )
    }

    pub fn invalid_header(header: &str, reason: &str) -> Self {
        Self::new(format!("Invalid {}: {}", header, reason), "INVALID_HEADER")
    }

    pub fn timestamp_expired() -> Self {
        Self::new(
            "Request timestamp is too old or in the future",
            "TIMESTAMP_EXPIRED",
        )
    }

    pub fn invalid_signature() -> Self {
        Self::new("Signature verification failed", "INVALID_SIGNATURE")
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

/// Authentication configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Maximum age of request timestamp in seconds.
    pub max_timestamp_age_secs: i64,
    /// Whether to allow requests from future timestamps (clock skew tolerance).
    pub allow_future_timestamp_secs: i64,
    /// Optional list of trusted public keys (if empty, any valid signature is accepted).
    pub trusted_keys: Vec<[u8; 32]>,
    /// Whether auth is required (false = bypass auth for testing).
    pub required: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            max_timestamp_age_secs: 300,     // 5 minutes
            allow_future_timestamp_secs: 60, // 1 minute clock skew
            trusted_keys: Vec::new(),
            required: true,
        }
    }
}

impl AuthConfig {
    /// Create config with auth disabled (for testing/development).
    pub fn disabled() -> Self {
        Self {
            required: false,
            ..Default::default()
        }
    }

    /// Add a trusted public key.
    pub fn with_trusted_key(mut self, key: [u8; 32]) -> Self {
        self.trusted_keys.push(key);
        self
    }

    /// Set whether auth is required.
    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

/// Verified authentication information extracted from request.
#[derive(Debug, Clone)]
pub struct AuthInfo {
    /// The public key that signed the request.
    pub public_key: [u8; 32],
    /// Base64-encoded public key for display.
    pub public_key_base64: String,
    /// The timestamp from the request.
    pub timestamp: DateTime<Utc>,
}

/// Authentication middleware.
///
/// Extracts and verifies Ed25519 signatures from request headers.
pub async fn auth_middleware(
    State(config): State<Arc<AuthConfig>>,
    headers: HeaderMap,
    request: Request,
) -> Result<(Request, AuthInfo), AuthError> {
    // If auth is not required, return a dummy auth info
    if !config.required {
        return Ok((
            request,
            AuthInfo {
                public_key: [0u8; 32],
                public_key_base64: "disabled".to_string(),
                timestamp: Utc::now(),
            },
        ));
    }

    // Extract headers
    let public_key_b64 = headers
        .get("X-Auth-PublicKey")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::missing_header("X-Auth-PublicKey"))?;

    let timestamp_str = headers
        .get("X-Auth-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::missing_header("X-Auth-Timestamp"))?;

    let signature_b64 = headers
        .get("X-Auth-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::missing_header("X-Auth-Signature"))?;

    // Decode public key
    let public_key_bytes = BASE64_STANDARD
        .decode(public_key_b64)
        .map_err(|_| AuthError::invalid_header("X-Auth-PublicKey", "invalid base64"))?;

    if public_key_bytes.len() != 32 {
        return Err(AuthError::invalid_header(
            "X-Auth-PublicKey",
            "must be 32 bytes",
        ));
    }

    let public_key: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| AuthError::invalid_header("X-Auth-PublicKey", "invalid length"))?;

    // Check if key is trusted (if trust list is configured)
    if !config.trusted_keys.is_empty() && !config.trusted_keys.contains(&public_key) {
        return Err(AuthError::new("Public key is not trusted", "UNTRUSTED_KEY"));
    }

    // Parse timestamp
    let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
        .map_err(|_| AuthError::invalid_header("X-Auth-Timestamp", "invalid ISO8601 format"))?
        .with_timezone(&Utc);

    // Verify timestamp is within acceptable range
    let now = Utc::now();
    let age = now.signed_duration_since(timestamp);

    if age > Duration::seconds(config.max_timestamp_age_secs) {
        return Err(AuthError::timestamp_expired());
    }

    if age < Duration::seconds(-config.allow_future_timestamp_secs) {
        return Err(AuthError::timestamp_expired());
    }

    // Decode signature
    let signature_bytes = BASE64_STANDARD
        .decode(signature_b64)
        .map_err(|_| AuthError::invalid_header("X-Auth-Signature", "invalid base64"))?;

    if signature_bytes.len() != 64 {
        return Err(AuthError::invalid_header(
            "X-Auth-Signature",
            "must be 64 bytes",
        ));
    }

    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| AuthError::invalid_header("X-Auth-Signature", "invalid length"))?;

    let signature = Signature::from_bytes(&signature_array);

    // Create verifying key
    let verifying_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| AuthError::invalid_header("X-Auth-PublicKey", "invalid Ed25519 key"))?;

    // Extract request parts for signing string (clone method/path before consuming)
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    // Read body for hashing
    let (parts, body) = request.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|_| AuthError::new("Failed to read request body", "BODY_READ_ERROR"))?
        .to_bytes();

    // Compute body hash
    let body_hash = hex::encode(Sha256::digest(&body_bytes));

    // Construct signing string
    let signing_string = format!("{}\n{}\n{}\n{}", method, path, timestamp_str, body_hash);

    // Verify signature
    verifying_key
        .verify(signing_string.as_bytes(), &signature)
        .map_err(|_| AuthError::invalid_signature())?;

    // Reconstruct request with body
    let request = Request::from_parts(parts, Body::from(body_bytes));

    let auth_info = AuthInfo {
        public_key,
        public_key_base64: public_key_b64.to_string(),
        timestamp,
    };

    Ok((request, auth_info))
}

/// Axum layer wrapper for auth middleware.
pub async fn auth_layer(
    State(config): State<Arc<AuthConfig>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let (request, auth_info) = auth_middleware(State(config), headers, request).await?;

    // Store auth info in request extensions for handlers to access
    let mut request = request;
    request.extensions_mut().insert(auth_info);

    Ok(next.run(request).await)
}

/// Helper to sign a request (for clients).
pub fn sign_request(
    method: &str,
    path: &str,
    body: &[u8],
    signing_key: &ed25519_dalek::SigningKey,
) -> SignedRequestHeaders {
    use ed25519_dalek::Signer;

    let timestamp = Utc::now().to_rfc3339();
    let body_hash = hex::encode(Sha256::digest(body));
    let signing_string = format!("{}\n{}\n{}\n{}", method, path, timestamp, body_hash);

    let signature = signing_key.sign(signing_string.as_bytes());
    let public_key = signing_key.verifying_key();

    SignedRequestHeaders {
        public_key: BASE64_STANDARD.encode(public_key.as_bytes()),
        timestamp,
        signature: BASE64_STANDARD.encode(signature.to_bytes()),
    }
}

/// Headers for a signed request.
#[derive(Debug, Clone)]
pub struct SignedRequestHeaders {
    pub public_key: String,
    pub timestamp: String,
    pub signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_and_verify_headers() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let method = "POST";
        let path = "/api/v1/workloads";
        let body = b"{\"name\": \"test\"}";

        let headers = sign_request(method, path, body, &signing_key);

        // Verify the headers can be decoded
        let pub_key_bytes = BASE64_STANDARD.decode(&headers.public_key).unwrap();
        assert_eq!(pub_key_bytes.len(), 32);

        let sig_bytes = BASE64_STANDARD.decode(&headers.signature).unwrap();
        assert_eq!(sig_bytes.len(), 64);

        // Parse timestamp
        let timestamp = DateTime::parse_from_rfc3339(&headers.timestamp).unwrap();
        assert!(
            timestamp
                .signed_duration_since(Utc::now())
                .num_seconds()
                .abs()
                < 5
        );
    }

    #[test]
    fn test_signature_verification() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let method = "GET";
        let path = "/api/v1/nodes";
        let body = b"";
        let timestamp = Utc::now().to_rfc3339();
        let body_hash = hex::encode(Sha256::digest(body));

        let signing_string = format!("{}\n{}\n{}\n{}", method, path, timestamp, body_hash);

        use ed25519_dalek::Signer;
        let signature = signing_key.sign(signing_string.as_bytes());

        // Verify signature
        assert!(verifying_key
            .verify(signing_string.as_bytes(), &signature)
            .is_ok());

        // Wrong message should fail
        assert!(verifying_key.verify(b"wrong message", &signature).is_err());
    }

    #[test]
    fn test_auth_config_defaults() {
        let config = AuthConfig::default();
        assert_eq!(config.max_timestamp_age_secs, 300);
        assert_eq!(config.allow_future_timestamp_secs, 60);
        assert!(config.trusted_keys.is_empty());
        assert!(config.required);
    }

    #[test]
    fn test_auth_config_disabled() {
        let config = AuthConfig::disabled();
        assert!(!config.required);
    }

    #[test]
    fn test_auth_error_responses() {
        let error = AuthError::missing_header("X-Auth-PublicKey");
        assert!(error.error.contains("X-Auth-PublicKey"));
        assert_eq!(error.code, "MISSING_HEADER");

        let error = AuthError::invalid_signature();
        assert_eq!(error.code, "INVALID_SIGNATURE");
    }
}
