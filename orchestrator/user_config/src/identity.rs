//! Ed25519 identity management.
//!
//! Provides cryptographic identity based on Ed25519 keys for user authentication
//! and signing operations.

use std::fmt;
use std::path::Path;

use base64::prelude::*;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};

/// User identity based on Ed25519 keypair.
#[derive(Clone)]
pub struct Identity {
    /// Unique identifier derived from public key.
    id: String,
    /// Ed25519 signing key (private).
    signing_key: SigningKey,
    /// Display name for the identity.
    display_name: Option<String>,
    /// Email associated with identity.
    email: Option<String>,
    /// When the identity was created.
    created_at: DateTime<Utc>,
}

impl Identity {
    /// Generate a new random identity.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let id = Self::derive_id(&verifying_key);

        Self {
            id,
            signing_key,
            display_name: None,
            email: None,
            created_at: Utc::now(),
        }
    }

    /// Create identity from existing signing key bytes.
    pub fn from_signing_key_bytes(bytes: &[u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(bytes);
        let verifying_key = signing_key.verifying_key();
        let id = Self::derive_id(&verifying_key);

        Ok(Self {
            id,
            signing_key,
            display_name: None,
            email: None,
            created_at: Utc::now(),
        })
    }

    /// Derive the identity ID from the public key.
    fn derive_id(verifying_key: &VerifyingKey) -> String {
        // Use first 16 bytes of public key as hex ID
        let bytes = verifying_key.as_bytes();
        format!("uid_{}", hex::encode(&bytes[..16]))
    }

    /// Get the unique identity ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the display name.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Set the display name.
    pub fn set_display_name(&mut self, name: impl Into<String>) {
        self.display_name = Some(name.into());
    }

    /// Get the email.
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Set the email.
    pub fn set_email(&mut self, email: impl Into<String>) {
        self.email = Some(email.into());
    }

    /// Get the creation timestamp.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get the public key bytes.
    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.signing_key.verifying_key().as_bytes()
    }

    /// Get the public key as base64.
    pub fn public_key_base64(&self) -> String {
        BASE64_STANDARD.encode(self.public_key_bytes())
    }

    /// Get the private key bytes.
    ///
    /// # Security
    /// Handle with care - this is sensitive key material.
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Get the verifying (public) key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Get a clone of the signing key.
    ///
    /// # Security
    /// Handle with care - this is sensitive key material.
    pub fn signing_key(&self) -> SigningKey {
        self.signing_key.clone()
    }

    /// Sign a message.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Sign a message and return signature bytes.
    pub fn sign_bytes(&self, message: &[u8]) -> [u8; 64] {
        self.sign(message).to_bytes()
    }

    /// Verify a signature against this identity's public key.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        self.signing_key
            .verifying_key()
            .verify(message, signature)
            .is_ok()
    }

    /// Verify signature bytes against this identity's public key.
    pub fn verify_bytes(&self, message: &[u8], signature_bytes: &[u8; 64]) -> bool {
        match Signature::from_bytes(signature_bytes) {
            sig => self.verify(message, &sig),
        }
    }

    /// Export identity metadata as TOML-serializable struct.
    pub fn to_metadata(&self) -> IdentityMetadata {
        IdentityMetadata {
            id: self.id.clone(),
            public_key: self.public_key_base64(),
            display_name: self.display_name.clone(),
            email: self.email.clone(),
            created_at: self.created_at,
        }
    }

    /// Create identity from metadata and private key.
    pub fn from_metadata_and_key(
        metadata: IdentityMetadata,
        private_key_bytes: &[u8; 32],
    ) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(private_key_bytes);

        // Verify the public key matches
        let expected_pub = BASE64_STANDARD.encode(signing_key.verifying_key().as_bytes());
        if expected_pub != metadata.public_key {
            return Err(ConfigError::InvalidKey(
                "Private key does not match public key in metadata".into(),
            ));
        }

        Ok(Self {
            id: metadata.id,
            signing_key,
            display_name: metadata.display_name,
            email: metadata.email,
            created_at: metadata.created_at,
        })
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("email", &self.email)
            .field("created_at", &self.created_at)
            .field("public_key", &self.public_key_base64())
            .finish()
    }
}

/// Serializable identity metadata (without private key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityMetadata {
    /// Unique identity ID.
    pub id: String,
    /// Base64-encoded public key.
    pub public_key: String,
    /// Optional display name.
    pub display_name: Option<String>,
    /// Optional email.
    pub email: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl IdentityMetadata {
    /// Verify a signature using the public key in metadata.
    pub fn verify(&self, message: &[u8], signature_bytes: &[u8; 64]) -> Result<bool> {
        let pub_bytes = BASE64_STANDARD.decode(&self.public_key)?;
        if pub_bytes.len() != 32 {
            return Err(ConfigError::InvalidKey("Invalid public key length".into()));
        }

        let pub_array: [u8; 32] = pub_bytes
            .try_into()
            .map_err(|_| ConfigError::InvalidKey("Failed to convert public key bytes".into()))?;

        let verifying_key = VerifyingKey::from_bytes(&pub_array)
            .map_err(|e| ConfigError::InvalidKey(format!("Invalid public key: {}", e)))?;

        let signature = Signature::from_bytes(signature_bytes);
        Ok(verifying_key.verify(message, &signature).is_ok())
    }
}

/// Identity file format (TOML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityFile {
    /// Configuration version.
    pub version: String,
    /// Identity metadata.
    pub identity: IdentityMetadata,
}

impl IdentityFile {
    /// Create from identity.
    pub fn from_identity(identity: &Identity) -> Self {
        Self {
            version: crate::CONFIG_VERSION.to_string(),
            identity: identity.to_metadata(),
        }
    }

    /// Load from TOML string.
    pub fn from_toml(content: &str) -> Result<Self> {
        Ok(toml::from_str(content)?)
    }

    /// Serialize to TOML string.
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Load from file.
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        Self::from_toml(&content)
    }

    /// Save to file.
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.to_toml()?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let identity = Identity::generate();
        assert!(identity.id().starts_with("uid_"));
        assert_eq!(identity.id().len(), 4 + 32); // "uid_" + 32 hex chars
    }

    #[test]
    fn test_identity_signing() {
        let identity = Identity::generate();
        let message = b"Hello, World!";

        let signature = identity.sign(message);
        assert!(identity.verify(message, &signature));

        // Wrong message should fail
        assert!(!identity.verify(b"Wrong message", &signature));
    }

    #[test]
    fn test_identity_sign_bytes() {
        let identity = Identity::generate();
        let message = b"Test message";

        let sig_bytes = identity.sign_bytes(message);
        assert!(identity.verify_bytes(message, &sig_bytes));
    }

    #[test]
    fn test_identity_from_key_bytes() {
        let identity1 = Identity::generate();
        let key_bytes = identity1.private_key_bytes();

        let identity2 = Identity::from_signing_key_bytes(&key_bytes).unwrap();
        assert_eq!(identity1.public_key_bytes(), identity2.public_key_bytes());
    }

    #[test]
    fn test_identity_metadata() {
        let mut identity = Identity::generate();
        identity.set_display_name("Test User");
        identity.set_email("test@example.com");

        let metadata = identity.to_metadata();
        assert_eq!(metadata.display_name.as_deref(), Some("Test User"));
        assert_eq!(metadata.email.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn test_metadata_verify() {
        let identity = Identity::generate();
        let metadata = identity.to_metadata();
        let message = b"Test message";

        let sig_bytes = identity.sign_bytes(message);
        assert!(metadata.verify(message, &sig_bytes).unwrap());
    }

    #[test]
    fn test_identity_file_serialization() {
        let identity = Identity::generate();
        let file = IdentityFile::from_identity(&identity);

        let toml = file.to_toml().unwrap();
        let loaded = IdentityFile::from_toml(&toml).unwrap();

        assert_eq!(loaded.identity.id, identity.id());
        assert_eq!(loaded.identity.public_key, identity.public_key_base64());
    }
}
