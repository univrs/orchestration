//! Main user configuration management.
//!
//! Provides a unified interface for all user configuration including identity,
//! trust policies, automation boundaries, and resource limits.

use serde::{Deserialize, Serialize};

use crate::automation::AutomationBoundary;
use crate::encryption::SecretStore;
use crate::error::{ConfigError, Result};
use crate::identity::{Identity, IdentityFile};
use crate::paths::ConfigPaths;
use crate::resources::ResourceLimits;
use crate::trust_policy::TrustPolicy;

/// Complete user configuration.
pub struct UserConfig {
    /// Configuration file paths.
    paths: ConfigPaths,
    /// User identity.
    identity: Identity,
    /// Secret store for encrypted data.
    secret_store: SecretStore,
    /// Trust policy.
    trust_policy: TrustPolicy,
    /// Automation boundaries.
    automation: AutomationBoundary,
    /// Resource limits.
    resources: ResourceLimits,
}

impl UserConfig {
    /// Load existing configuration or create new.
    pub async fn load_or_create() -> Result<Self> {
        let paths = ConfigPaths::new()?;
        Self::load_or_create_with_paths(paths).await
    }

    /// Load or create with custom paths.
    pub async fn load_or_create_with_paths(paths: ConfigPaths) -> Result<Self> {
        // Ensure directories exist
        paths.ensure_dirs().await?;

        if paths.config_exists() {
            Self::load_with_paths(paths).await
        } else {
            Self::create_new_with_paths(paths).await
        }
    }

    /// Load existing configuration.
    pub async fn load() -> Result<Self> {
        let paths = ConfigPaths::new()?;
        Self::load_with_paths(paths).await
    }

    /// Load with custom paths.
    pub async fn load_with_paths(paths: ConfigPaths) -> Result<Self> {
        // Load identity metadata
        let identity_file = IdentityFile::load(paths.identity_file()).await?;

        // Load age identity for secret store
        let secret_store = SecretStore::load_identity(paths.private_key_file(), paths.clone()).await?;

        // Load and decrypt Ed25519 private key
        let encrypted_key = tokio::fs::read(paths.secret_file("ed25519_key")).await?;
        let key_bytes = secret_store.decrypt(&encrypted_key)?;
        if key_bytes.len() != 32 {
            return Err(ConfigError::InvalidKey("Invalid Ed25519 key length".into()));
        }
        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
            ConfigError::InvalidKey("Failed to convert key bytes".into())
        })?;

        // Reconstruct identity
        let identity = Identity::from_metadata_and_key(identity_file.identity, &key_array)?;

        // Load trust policy
        let trust_policy = if paths.trust_policy_file().exists() {
            TrustPolicy::load(paths.trust_policy_file()).await?
        } else {
            TrustPolicy::default()
        };

        // Load automation and resources from trust policy file or use defaults
        let (automation, resources) = Self::load_extended_config(&paths).await?;

        Ok(Self {
            paths,
            identity,
            secret_store,
            trust_policy,
            automation,
            resources,
        })
    }

    /// Create new configuration.
    pub async fn create_new() -> Result<Self> {
        let paths = ConfigPaths::new()?;
        Self::create_new_with_paths(paths).await
    }

    /// Create new with custom paths.
    pub async fn create_new_with_paths(paths: ConfigPaths) -> Result<Self> {
        // Ensure directories exist
        paths.ensure_dirs().await?;

        // Generate new identity
        let identity = Identity::generate();

        // Generate new secret store
        let secret_store = SecretStore::generate(paths.clone());

        // Save age identity (encrypted key store key)
        secret_store.save_identity(paths.private_key_file()).await?;

        // Encrypt and save Ed25519 private key
        let ed25519_key = identity.private_key_bytes();
        let encrypted_key = secret_store.encrypt(&ed25519_key)?;
        tokio::fs::write(paths.secret_file("ed25519_key"), &encrypted_key).await?;

        // Set restrictive permissions on key file
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(paths.secret_file("ed25519_key"), perms).await?;
        }

        // Save identity metadata
        let identity_file = IdentityFile::from_identity(&identity);
        identity_file.save(paths.identity_file()).await?;

        // Save public key separately for easy sharing
        tokio::fs::write(paths.public_key_file(), identity.public_key_base64()).await?;

        // Create default trust policy
        let trust_policy = TrustPolicy::default();
        trust_policy.save(paths.trust_policy_file()).await?;

        // Create default automation and resources
        let automation = AutomationBoundary::default();
        let resources = ResourceLimits::default();
        Self::save_extended_config(&paths, &automation, &resources).await?;

        tracing::info!(
            identity_id = %identity.id(),
            config_dir = %paths.config_dir().display(),
            "Created new user configuration"
        );

        Ok(Self {
            paths,
            identity,
            secret_store,
            trust_policy,
            automation,
            resources,
        })
    }

    /// Load extended configuration (automation, resources).
    async fn load_extended_config(paths: &ConfigPaths) -> Result<(AutomationBoundary, ResourceLimits)> {
        let extended_path = paths.config_dir().join("settings.toml");

        if extended_path.exists() {
            let content = tokio::fs::read_to_string(&extended_path).await?;
            let settings: ExtendedSettings = toml::from_str(&content)?;
            Ok((settings.automation, settings.resources))
        } else {
            Ok((AutomationBoundary::default(), ResourceLimits::default()))
        }
    }

    /// Save extended configuration.
    async fn save_extended_config(
        paths: &ConfigPaths,
        automation: &AutomationBoundary,
        resources: &ResourceLimits,
    ) -> Result<()> {
        let settings = ExtendedSettings {
            version: crate::CONFIG_VERSION.to_string(),
            automation: automation.clone(),
            resources: resources.clone(),
        };

        let content = toml::to_string_pretty(&settings)?;
        let extended_path = paths.config_dir().join("settings.toml");
        tokio::fs::write(extended_path, content).await?;
        Ok(())
    }

    /// Get the identity.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Get the secret store.
    pub fn secret_store(&self) -> &SecretStore {
        &self.secret_store
    }

    /// Get the trust policy.
    pub fn trust_policy(&self) -> &TrustPolicy {
        &self.trust_policy
    }

    /// Get mutable trust policy.
    pub fn trust_policy_mut(&mut self) -> &mut TrustPolicy {
        &mut self.trust_policy
    }

    /// Get the automation boundary.
    pub fn automation(&self) -> &AutomationBoundary {
        &self.automation
    }

    /// Get mutable automation boundary.
    pub fn automation_mut(&mut self) -> &mut AutomationBoundary {
        &mut self.automation
    }

    /// Get the resource limits.
    pub fn resources(&self) -> &ResourceLimits {
        &self.resources
    }

    /// Get mutable resource limits.
    pub fn resources_mut(&mut self) -> &mut ResourceLimits {
        &mut self.resources
    }

    /// Get the configuration paths.
    pub fn paths(&self) -> &ConfigPaths {
        &self.paths
    }

    /// Save all configuration.
    pub async fn save(&self) -> Result<()> {
        // Save identity metadata
        let identity_file = IdentityFile::from_identity(&self.identity);
        identity_file.save(self.paths.identity_file()).await?;

        // Save trust policy
        self.trust_policy.save(self.paths.trust_policy_file()).await?;

        // Save extended settings
        Self::save_extended_config(&self.paths, &self.automation, &self.resources).await?;

        Ok(())
    }

    /// Store a secret.
    pub async fn store_secret(&self, name: &str, value: &[u8]) -> Result<()> {
        self.secret_store.store_secret(name, value).await
    }

    /// Get a secret.
    pub async fn get_secret(&self, name: &str) -> Result<Vec<u8>> {
        self.secret_store.get_secret(name).await
    }

    /// Check if a secret exists.
    pub fn secret_exists(&self, name: &str) -> bool {
        self.secret_store.secret_exists(name)
    }

    /// Delete a secret.
    pub async fn delete_secret(&self, name: &str) -> Result<()> {
        self.secret_store.delete_secret(name).await
    }

    /// List all secrets.
    pub async fn list_secrets(&self) -> Result<Vec<String>> {
        self.secret_store.list_secrets().await
    }

    /// Export public identity for sharing.
    pub fn export_public_identity(&self) -> PublicIdentity {
        PublicIdentity {
            id: self.identity.id().to_string(),
            public_key: self.identity.public_key_base64(),
            display_name: self.identity.display_name().map(String::from),
            age_recipient: self.secret_store.recipient_string(),
        }
    }

    /// Sign a message with the identity.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.identity.sign_bytes(message)
    }

    /// Verify a signature.
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        self.identity.verify_bytes(message, signature)
    }
}

impl std::fmt::Debug for UserConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserConfig")
            .field("identity_id", &self.identity.id())
            .field("paths", &self.paths)
            .field("trust_policy_version", &self.trust_policy.version)
            .field("automation_level", &self.automation.level)
            .finish()
    }
}

/// Extended settings file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtendedSettings {
    version: String,
    automation: AutomationBoundary,
    resources: ResourceLimits,
}

/// Public identity for sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIdentity {
    /// Identity ID.
    pub id: String,
    /// Base64-encoded Ed25519 public key.
    pub public_key: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Age recipient for encryption.
    pub age_recipient: String,
}

impl PublicIdentity {
    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn test_config() -> (UserConfig, TempDir) {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());
        let config = UserConfig::create_new_with_paths(paths).await.unwrap();
        (config, temp)
    }

    #[tokio::test]
    async fn test_create_new_config() {
        let (config, _temp) = test_config().await;
        assert!(config.identity().id().starts_with("uid_"));
    }

    #[tokio::test]
    async fn test_load_existing_config() {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());

        // Create new config
        let config1 = UserConfig::create_new_with_paths(paths.clone()).await.unwrap();
        let id1 = config1.identity().id().to_string();

        // Load existing config
        let config2 = UserConfig::load_with_paths(paths).await.unwrap();
        assert_eq!(config2.identity().id(), id1);
    }

    #[tokio::test]
    async fn test_store_and_get_secret() {
        let (config, _temp) = test_config().await;

        config.store_secret("api_key", b"secret123").await.unwrap();
        assert!(config.secret_exists("api_key"));

        let retrieved = config.get_secret("api_key").await.unwrap();
        assert_eq!(retrieved, b"secret123");
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let (config, _temp) = test_config().await;

        let message = b"Test message";
        let signature = config.sign(message);
        assert!(config.verify(message, &signature));
        assert!(!config.verify(b"Wrong message", &signature));
    }

    #[tokio::test]
    async fn test_export_public_identity() {
        let (config, _temp) = test_config().await;

        let public = config.export_public_identity();
        assert_eq!(public.id, config.identity().id());
        assert!(public.age_recipient.starts_with("age1"));
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());

        // Create and modify config
        let mut config = UserConfig::create_new_with_paths(paths.clone()).await.unwrap();
        config.trust_policy_mut().network.allow_outbound = false;
        config.save().await.unwrap();

        // Reload and verify
        let loaded = UserConfig::load_with_paths(paths).await.unwrap();
        assert!(!loaded.trust_policy().network.allow_outbound);
    }
}
