//! HTTP client for REST API communication with Ed25519 request signing.

#![allow(dead_code)]

use base64::prelude::*;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use reqwest::{Client, Method, RequestBuilder, Response};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use user_config::UserConfig;

use crate::error::{CliError, Result};

/// API client with request signing.
pub struct ApiClient {
    client: Client,
    base_url: String,
    signing_key: Option<SigningKey>,
}

impl ApiClient {
    /// Create a new API client.
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            signing_key: None,
        }
    }

    /// Create an authenticated API client using user config identity.
    pub async fn authenticated(base_url: &str) -> Result<Self> {
        let config = UserConfig::load_or_create().await?;
        let identity = config.identity();

        // Get the signing key from identity
        let signing_key = identity.signing_key();

        Ok(Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            signing_key: Some(signing_key),
        })
    }

    /// Set the signing key directly (for testing).
    pub fn with_signing_key(mut self, key: SigningKey) -> Self {
        self.signing_key = Some(key);
        self
    }

    /// Sign a request and add authentication headers.
    fn sign_request(&self, method: &str, path: &str, body: &[u8]) -> Option<SignedHeaders> {
        let signing_key = self.signing_key.as_ref()?;

        let timestamp = Utc::now().to_rfc3339();
        let body_hash = hex::encode(Sha256::digest(body));
        let signing_string = format!("{}\n{}\n{}\n{}", method, path, timestamp, body_hash);

        let signature = signing_key.sign(signing_string.as_bytes());
        let public_key = signing_key.verifying_key();

        Some(SignedHeaders {
            public_key: BASE64_STANDARD.encode(public_key.as_bytes()),
            timestamp,
            signature: BASE64_STANDARD.encode(signature.to_bytes()),
        })
    }

    /// Apply authentication headers to a request builder.
    fn apply_auth(
        &self,
        builder: RequestBuilder,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> RequestBuilder {
        if let Some(headers) = self.sign_request(method, path, body) {
            builder
                .header("X-Auth-PublicKey", headers.public_key)
                .header("X-Auth-Timestamp", headers.timestamp)
                .header("X-Auth-Signature", headers.signature)
        } else {
            builder
        }
    }

    /// Build the full URL for a path.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Perform a GET request.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.url(path);
        let builder = self.client.get(&url);
        let builder = self.apply_auth(builder, "GET", path, &[]);

        let response = builder.send().await?;
        self.handle_response(response).await
    }

    /// Perform a POST request with JSON body.
    pub async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = self.url(path);
        let body_bytes = serde_json::to_vec(body)?;
        let builder = self.client.post(&url).body(body_bytes.clone());
        let builder = self.apply_auth(builder, "POST", path, &body_bytes);
        let builder = builder.header("Content-Type", "application/json");

        let response = builder.send().await?;
        self.handle_response(response).await
    }

    /// Perform a PUT request with JSON body.
    pub async fn put<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = self.url(path);
        let body_bytes = serde_json::to_vec(body)?;
        let builder = self.client.put(&url).body(body_bytes.clone());
        let builder = self.apply_auth(builder, "PUT", path, &body_bytes);
        let builder = builder.header("Content-Type", "application/json");

        let response = builder.send().await?;
        self.handle_response(response).await
    }

    /// Perform a DELETE request.
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = self.url(path);
        let builder = self.client.delete(&url);
        let builder = self.apply_auth(builder, "DELETE", path, &[]);

        let response = builder.send().await?;
        self.handle_empty_response(response).await
    }

    /// Perform a raw request and return the response.
    pub async fn request(&self, method: Method, path: &str) -> Result<Response> {
        let url = self.url(path);
        let builder = self.client.request(method.clone(), &url);
        let method_str = method.as_str();
        let builder = self.apply_auth(builder, method_str, path, &[]);

        Ok(builder.send().await?)
    }

    /// Handle a JSON response.
    async fn handle_response<T: DeserializeOwned>(&self, response: Response) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            Ok(response.json().await?)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CliError::api_error(format!(
                "Request failed with status {}: {}",
                status, error_text
            )))
        }
    }

    /// Handle an empty response (for DELETE, etc.).
    async fn handle_empty_response(&self, response: Response) -> Result<()> {
        let status = response.status();

        if status.is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CliError::api_error(format!(
                "Request failed with status {}: {}",
                status, error_text
            )))
        }
    }
}

/// Signed request headers.
struct SignedHeaders {
    public_key: String,
    timestamp: String,
    signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_request() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let client = ApiClient::new("http://localhost:9090").with_signing_key(signing_key);

        let headers = client.sign_request("GET", "/api/v1/nodes", &[]);
        assert!(headers.is_some());

        let headers = headers.unwrap();
        assert!(!headers.public_key.is_empty());
        assert!(!headers.timestamp.is_empty());
        assert!(!headers.signature.is_empty());
    }

    #[test]
    fn test_url_construction() {
        let client = ApiClient::new("http://localhost:9090/");
        assert_eq!(
            client.url("/api/v1/nodes"),
            "http://localhost:9090/api/v1/nodes"
        );

        let client = ApiClient::new("http://localhost:9090");
        assert_eq!(
            client.url("/api/v1/nodes"),
            "http://localhost:9090/api/v1/nodes"
        );
    }
}
