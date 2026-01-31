//! Age encryption for secrets.
//!
//! Provides secure storage of secrets using age encryption with X25519 keys.

use std::io::{Read, Write};
use std::path::Path;

use age::secrecy::ExposeSecret;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};
use crate::paths::ConfigPaths;

/// Secret store for managing encrypted secrets.
pub struct SecretStore {
    /// Age identity for decryption.
    identity: age::x25519::Identity,
    /// Age recipient for encryption (derived from identity).
    recipient: age::x25519::Recipient,
    /// Configuration paths.
    paths: ConfigPaths,
}

impl SecretStore {
    /// Create a new secret store with a generated key.
    pub fn generate(paths: ConfigPaths) -> Self {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();

        Self {
            identity,
            recipient,
            paths,
        }
    }

    /// Create from an existing age identity string.
    pub fn from_identity_str(identity_str: &str, paths: ConfigPaths) -> Result<Self> {
        let identity: age::x25519::Identity = identity_str
            .parse()
            .map_err(|e| ConfigError::encryption(format!("Invalid age identity: {}", e)))?;

        let recipient = identity.to_public();

        Ok(Self {
            identity,
            recipient,
            paths,
        })
    }

    /// Get the age identity string (private key).
    ///
    /// # Security
    /// Handle with care - this is sensitive key material.
    pub fn identity_string(&self) -> SecretString {
        SecretString::new(self.identity.to_string().expose_secret().clone())
    }

    /// Get the age recipient string (public key).
    pub fn recipient_string(&self) -> String {
        self.recipient.to_string()
    }

    /// Encrypt data for storage.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let encryptor = age::Encryptor::with_recipients(vec![Box::new(self.recipient.clone())])
            .ok_or_else(|| ConfigError::encryption("Failed to create encryptor: no recipients"))?;

        let mut encrypted = vec![];
        let mut writer = encryptor
            .wrap_output(&mut encrypted)
            .map_err(|e| ConfigError::encryption(format!("Failed to wrap output: {}", e)))?;

        writer
            .write_all(plaintext)
            .map_err(|e| ConfigError::encryption(format!("Failed to write: {}", e)))?;

        writer
            .finish()
            .map_err(|e| ConfigError::encryption(format!("Failed to finish encryption: {}", e)))?;

        Ok(encrypted)
    }

    /// Encrypt data with ASCII armor (for text files).
    pub fn encrypt_armor(&self, plaintext: &[u8]) -> Result<String> {
        let encryptor = age::Encryptor::with_recipients(vec![Box::new(self.recipient.clone())])
            .ok_or_else(|| ConfigError::encryption("Failed to create encryptor: no recipients"))?;

        let mut encrypted = vec![];
        let armor_writer =
            age::armor::ArmoredWriter::wrap_output(&mut encrypted, age::armor::Format::AsciiArmor)
                .map_err(|e| {
                    ConfigError::encryption(format!("Failed to create armor writer: {}", e))
                })?;

        let mut writer = encryptor
            .wrap_output(armor_writer)
            .map_err(|e| ConfigError::encryption(format!("Failed to wrap output: {}", e)))?;

        writer
            .write_all(plaintext)
            .map_err(|e| ConfigError::encryption(format!("Failed to write: {}", e)))?;

        let armor_writer = writer
            .finish()
            .map_err(|e| ConfigError::encryption(format!("Failed to finish encryption: {}", e)))?;

        armor_writer
            .finish()
            .map_err(|e| ConfigError::encryption(format!("Failed to finish armor: {}", e)))?;

        String::from_utf8(encrypted)
            .map_err(|e| ConfigError::encryption(format!("Invalid UTF-8 in armored output: {}", e)))
    }

    /// Decrypt data.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let decryptor = match age::Decryptor::new(ciphertext)
            .map_err(|e| ConfigError::encryption(format!("Failed to create decryptor: {}", e)))?
        {
            age::Decryptor::Recipients(d) => d,
            _ => return Err(ConfigError::encryption("Unexpected decryptor type")),
        };

        let mut decrypted = vec![];
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|e| ConfigError::encryption(format!("Failed to decrypt: {}", e)))?;

        reader.read_to_end(&mut decrypted).map_err(|e| {
            ConfigError::encryption(format!("Failed to read decrypted data: {}", e))
        })?;

        Ok(decrypted)
    }

    /// Decrypt armored data.
    pub fn decrypt_armor(&self, armored: &str) -> Result<Vec<u8>> {
        let armor_reader = age::armor::ArmoredReader::new(armored.as_bytes());

        let decryptor = match age::Decryptor::new(armor_reader)
            .map_err(|e| ConfigError::encryption(format!("Failed to create decryptor: {}", e)))?
        {
            age::Decryptor::Recipients(d) => d,
            _ => return Err(ConfigError::encryption("Unexpected decryptor type")),
        };

        let mut decrypted = vec![];
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|e| ConfigError::encryption(format!("Failed to decrypt: {}", e)))?;

        reader.read_to_end(&mut decrypted).map_err(|e| {
            ConfigError::encryption(format!("Failed to read decrypted data: {}", e))
        })?;

        Ok(decrypted)
    }

    /// Store a secret by name.
    pub async fn store_secret(&self, name: &str, value: &[u8]) -> Result<()> {
        let encrypted = self.encrypt_armor(value)?;
        let path = self.paths.secret_file(name);

        // Ensure secrets directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&path, encrypted).await?;

        // Set restrictive permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(&path, perms).await?;
        }

        Ok(())
    }

    /// Retrieve a secret by name.
    pub async fn get_secret(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.paths.secret_file(name);
        let armored = tokio::fs::read_to_string(&path).await?;
        self.decrypt_armor(&armored)
    }

    /// Check if a secret exists.
    pub fn secret_exists(&self, name: &str) -> bool {
        self.paths.secret_file(name).exists()
    }

    /// Delete a secret.
    pub async fn delete_secret(&self, name: &str) -> Result<()> {
        let path = self.paths.secret_file(name);
        if path.exists() {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    /// List all stored secrets.
    pub async fn list_secrets(&self) -> Result<Vec<String>> {
        let secrets_dir = self.paths.secrets_dir();
        if !secrets_dir.exists() {
            return Ok(vec![]);
        }

        let mut secrets = vec![];
        let mut entries = tokio::fs::read_dir(&secrets_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "age" {
                    if let Some(stem) = path.file_stem() {
                        secrets.push(stem.to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(secrets)
    }

    /// Save the age identity to a file.
    pub async fn save_identity(&self, path: impl AsRef<Path>) -> Result<()> {
        let identity_str = self.identity.to_string();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Write to temp file first, then rename
            let path = path.as_ref();
            let temp_path = path.with_extension("tmp");

            tokio::fs::write(&temp_path, identity_str.expose_secret()).await?;
            let perms = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(&temp_path, perms).await?;
            tokio::fs::rename(&temp_path, path).await?;
        }

        #[cfg(not(unix))]
        {
            tokio::fs::write(path, identity_str.expose_secret()).await?;
        }

        Ok(())
    }

    /// Load age identity from a file.
    pub async fn load_identity(path: impl AsRef<Path>, paths: ConfigPaths) -> Result<Self> {
        let identity_str = tokio::fs::read_to_string(path).await?;
        Self::from_identity_str(identity_str.trim(), paths)
    }
}

impl std::fmt::Debug for SecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretStore")
            .field("recipient", &self.recipient_string())
            .field("paths", &self.paths)
            .finish()
    }
}

/// Encrypted secret value wrapper.
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedSecret {
    /// Armored ciphertext.
    pub ciphertext: String,
    /// When the secret was stored.
    pub stored_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_paths() -> ConfigPaths {
        let temp = TempDir::new().unwrap();
        ConfigPaths::with_base(temp.keep())
    }

    #[test]
    fn test_encrypt_decrypt() {
        let store = SecretStore::generate(test_paths());
        let plaintext = b"Hello, World!";

        let ciphertext = store.encrypt(plaintext).unwrap();
        let decrypted = store.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_armor() {
        let store = SecretStore::generate(test_paths());
        let plaintext = b"Secret message";

        let armored = store.encrypt_armor(plaintext).unwrap();
        assert!(armored.starts_with("-----BEGIN AGE ENCRYPTED FILE-----"));

        let decrypted = store.decrypt_armor(&armored).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_identity_roundtrip() {
        let paths = test_paths();
        let store1 = SecretStore::generate(paths.clone());
        let identity_str = store1.identity_string();

        let store2 = SecretStore::from_identity_str(identity_str.expose_secret(), paths).unwrap();
        assert_eq!(store1.recipient_string(), store2.recipient_string());
    }

    #[tokio::test]
    async fn test_store_get_secret() {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());
        paths.ensure_dirs().await.unwrap();

        let store = SecretStore::generate(paths);
        let secret_value = b"my-secret-value";

        store
            .store_secret("test-secret", secret_value)
            .await
            .unwrap();
        assert!(store.secret_exists("test-secret"));

        let retrieved = store.get_secret("test-secret").await.unwrap();
        assert_eq!(secret_value.as_slice(), retrieved.as_slice());
    }

    #[tokio::test]
    async fn test_list_secrets() {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());
        paths.ensure_dirs().await.unwrap();

        let store = SecretStore::generate(paths);

        store.store_secret("secret1", b"value1").await.unwrap();
        store.store_secret("secret2", b"value2").await.unwrap();

        let secrets = store.list_secrets().await.unwrap();
        assert_eq!(secrets.len(), 2);
        assert!(secrets.contains(&"secret1".to_string()));
        assert!(secrets.contains(&"secret2".to_string()));
    }

    #[tokio::test]
    async fn test_delete_secret() {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());
        paths.ensure_dirs().await.unwrap();

        let store = SecretStore::generate(paths);

        store.store_secret("to-delete", b"value").await.unwrap();
        assert!(store.secret_exists("to-delete"));

        store.delete_secret("to-delete").await.unwrap();
        assert!(!store.secret_exists("to-delete"));
    }
}
