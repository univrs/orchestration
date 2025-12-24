//! XDG-compliant configuration paths.
//!
//! Provides paths for configuration, data, and secrets following XDG Base Directory Specification.

use std::path::{Path, PathBuf};

use crate::{error::Result, ConfigError, APP_NAME};

/// Configuration paths following XDG Base Directory Specification.
///
/// Default locations:
/// - Config: `~/.config/univrs/`
/// - Data: `~/.local/share/univrs/`
/// - Cache: `~/.cache/univrs/`
/// - Secrets: `~/.config/univrs/secrets/`
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// Base configuration directory.
    config_dir: PathBuf,
    /// Data directory for persistent data.
    data_dir: PathBuf,
    /// Cache directory for temporary data.
    cache_dir: PathBuf,
}

impl ConfigPaths {
    /// Create paths using XDG defaults.
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::PathError("Could not determine config directory".into()))?
            .join(APP_NAME);

        let data_dir = dirs::data_dir()
            .ok_or_else(|| ConfigError::PathError("Could not determine data directory".into()))?
            .join(APP_NAME);

        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| ConfigError::PathError("Could not determine cache directory".into()))?
            .join(APP_NAME);

        Ok(Self {
            config_dir,
            data_dir,
            cache_dir,
        })
    }

    /// Create paths with a custom base directory (for testing).
    pub fn with_base(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        Self {
            config_dir: base.join("config"),
            data_dir: base.join("data"),
            cache_dir: base.join("cache"),
        }
    }

    /// Get the base configuration directory.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Get the data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Get the cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the identity configuration file path.
    pub fn identity_file(&self) -> PathBuf {
        self.config_dir.join("identity.toml")
    }

    /// Get the trust policy configuration file path.
    pub fn trust_policy_file(&self) -> PathBuf {
        self.config_dir.join("trust_policy.toml")
    }

    /// Get the secrets directory path.
    pub fn secrets_dir(&self) -> PathBuf {
        self.config_dir.join("secrets")
    }

    /// Get the private key file path (within secrets).
    pub fn private_key_file(&self) -> PathBuf {
        self.secrets_dir().join("identity.key.age")
    }

    /// Get the public key file path.
    pub fn public_key_file(&self) -> PathBuf {
        self.config_dir.join("identity.pub")
    }

    /// Get a path for a named secret.
    pub fn secret_file(&self, name: &str) -> PathBuf {
        self.secrets_dir().join(format!("{}.age", name))
    }

    /// Get the state file path for persistent state.
    pub fn state_file(&self) -> PathBuf {
        self.data_dir.join("state.json")
    }

    /// Get the audit log file path.
    pub fn audit_log_file(&self) -> PathBuf {
        self.data_dir.join("audit.log")
    }

    /// Ensure all directories exist with proper permissions.
    pub async fn ensure_dirs(&self) -> Result<()> {
        // Create directories
        tokio::fs::create_dir_all(&self.config_dir).await?;
        tokio::fs::create_dir_all(&self.data_dir).await?;
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        tokio::fs::create_dir_all(self.secrets_dir()).await?;

        // Set restrictive permissions on secrets directory (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            tokio::fs::set_permissions(self.secrets_dir(), perms).await?;
        }

        Ok(())
    }

    /// Check if configuration exists.
    pub fn config_exists(&self) -> bool {
        self.identity_file().exists()
    }
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new().expect("Failed to determine XDG paths")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_paths_with_base() {
        let base = PathBuf::from("/tmp/test");
        let paths = ConfigPaths::with_base(&base);

        assert_eq!(paths.config_dir(), base.join("config"));
        assert_eq!(paths.data_dir(), base.join("data"));
        assert_eq!(paths.cache_dir(), base.join("cache"));
    }

    #[test]
    fn test_file_paths() {
        let base = PathBuf::from("/tmp/test");
        let paths = ConfigPaths::with_base(&base);

        assert_eq!(
            paths.identity_file(),
            base.join("config/identity.toml")
        );
        assert_eq!(
            paths.trust_policy_file(),
            base.join("config/trust_policy.toml")
        );
        assert_eq!(
            paths.secrets_dir(),
            base.join("config/secrets")
        );
        assert_eq!(
            paths.private_key_file(),
            base.join("config/secrets/identity.key.age")
        );
    }

    #[tokio::test]
    async fn test_ensure_dirs() {
        let temp = TempDir::new().unwrap();
        let paths = ConfigPaths::with_base(temp.path());

        paths.ensure_dirs().await.unwrap();

        assert!(paths.config_dir().exists());
        assert!(paths.data_dir().exists());
        assert!(paths.cache_dir().exists());
        assert!(paths.secrets_dir().exists());
    }
}
