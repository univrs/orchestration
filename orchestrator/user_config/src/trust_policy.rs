//! Trust policy configuration.
//!
//! Defines what actions are allowed, which domains/resources are trusted,
//! and what level of human approval is required for various operations.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Trust policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPolicy {
    /// Configuration version.
    #[serde(default = "default_version")]
    pub version: String,

    /// Network access policies.
    #[serde(default)]
    pub network: NetworkPolicy,

    /// File system access policies.
    #[serde(default)]
    pub filesystem: FilesystemPolicy,

    /// Execution policies.
    #[serde(default)]
    pub execution: ExecutionPolicy,

    /// Trusted identities (public keys or identity IDs).
    #[serde(default)]
    pub trusted_identities: Vec<TrustedIdentity>,

    /// Approval requirements.
    #[serde(default)]
    pub approval: ApprovalPolicy,
}

fn default_version() -> String {
    crate::CONFIG_VERSION.to_string()
}

impl Default for TrustPolicy {
    fn default() -> Self {
        Self {
            version: default_version(),
            network: NetworkPolicy::default(),
            filesystem: FilesystemPolicy::default(),
            execution: ExecutionPolicy::default(),
            trusted_identities: vec![],
            approval: ApprovalPolicy::default(),
        }
    }
}

impl TrustPolicy {
    /// Create a restrictive (paranoid) policy.
    pub fn restrictive() -> Self {
        Self {
            version: default_version(),
            network: NetworkPolicy {
                allow_outbound: false,
                allowed_domains: vec![],
                blocked_domains: vec!["*".to_string()],
                allow_localhost: false,
            },
            filesystem: FilesystemPolicy {
                allowed_read_paths: vec![],
                allowed_write_paths: vec![],
                blocked_paths: vec!["/".to_string()],
                allow_home_access: false,
                allow_temp_access: true,
            },
            execution: ExecutionPolicy {
                allow_shell_commands: false,
                allowed_commands: vec![],
                blocked_commands: vec!["*".to_string()],
                allow_background_processes: false,
                max_execution_time_secs: 60,
            },
            trusted_identities: vec![],
            approval: ApprovalPolicy {
                require_approval_for_network: true,
                require_approval_for_writes: true,
                require_approval_for_execution: true,
                require_approval_for_secrets: true,
                auto_approve_trusted: false,
            },
        }
    }

    /// Create a permissive (development) policy.
    pub fn permissive() -> Self {
        Self {
            version: default_version(),
            network: NetworkPolicy {
                allow_outbound: true,
                allowed_domains: vec!["*".to_string()],
                blocked_domains: vec![],
                allow_localhost: true,
            },
            filesystem: FilesystemPolicy {
                allowed_read_paths: vec!["*".to_string()],
                allowed_write_paths: vec!["~/*".to_string()],
                blocked_paths: vec![],
                allow_home_access: true,
                allow_temp_access: true,
            },
            execution: ExecutionPolicy {
                allow_shell_commands: true,
                allowed_commands: vec!["*".to_string()],
                blocked_commands: vec![],
                allow_background_processes: true,
                max_execution_time_secs: 3600,
            },
            trusted_identities: vec![],
            approval: ApprovalPolicy {
                require_approval_for_network: false,
                require_approval_for_writes: false,
                require_approval_for_execution: false,
                require_approval_for_secrets: true,
                auto_approve_trusted: true,
            },
        }
    }

    /// Check if network access to a domain is allowed.
    pub fn allows_network_access(&self, domain: &str) -> bool {
        if !self.network.allow_outbound {
            return false;
        }

        // Check blocked domains first
        for blocked in &self.network.blocked_domains {
            if Self::matches_pattern(blocked, domain) {
                return false;
            }
        }

        // Check allowed domains
        if self.network.allowed_domains.is_empty() {
            return true; // No allowlist means allow all (except blocked)
        }

        for allowed in &self.network.allowed_domains {
            if Self::matches_pattern(allowed, domain) {
                return true;
            }
        }

        false
    }

    /// Check if reading a path is allowed.
    pub fn allows_read(&self, path: &str) -> bool {
        // Check blocked paths first
        for blocked in &self.filesystem.blocked_paths {
            if Self::matches_path_pattern(blocked, path) {
                return false;
            }
        }

        // Check special paths
        if path.starts_with("/tmp") && self.filesystem.allow_temp_access {
            return true;
        }

        if (path.starts_with("~/")
            || path.starts_with(
                &dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            ))
            && self.filesystem.allow_home_access
        {
            return true;
        }

        // Check allowed paths
        for allowed in &self.filesystem.allowed_read_paths {
            if Self::matches_path_pattern(allowed, path) {
                return true;
            }
        }

        false
    }

    /// Check if writing to a path is allowed.
    pub fn allows_write(&self, path: &str) -> bool {
        // Check blocked paths first
        for blocked in &self.filesystem.blocked_paths {
            if Self::matches_path_pattern(blocked, path) {
                return false;
            }
        }

        // Check special paths
        if path.starts_with("/tmp") && self.filesystem.allow_temp_access {
            return true;
        }

        // Check allowed paths
        for allowed in &self.filesystem.allowed_write_paths {
            if Self::matches_path_pattern(allowed, path) {
                return true;
            }
        }

        false
    }

    /// Check if executing a command is allowed.
    pub fn allows_command(&self, command: &str) -> bool {
        if !self.execution.allow_shell_commands {
            return false;
        }

        // Extract the base command (first word)
        let base_command = command.split_whitespace().next().unwrap_or(command);

        // Check blocked commands first
        for blocked in &self.execution.blocked_commands {
            if Self::matches_pattern(blocked, base_command) {
                return false;
            }
        }

        // Check allowed commands
        if self.execution.allowed_commands.is_empty() {
            return true;
        }

        for allowed in &self.execution.allowed_commands {
            if Self::matches_pattern(allowed, base_command) {
                return true;
            }
        }

        false
    }

    /// Check if an identity is trusted.
    pub fn is_identity_trusted(&self, identity_id: &str) -> bool {
        self.trusted_identities
            .iter()
            .any(|t| t.id == identity_id && !t.revoked)
    }

    /// Simple pattern matching with wildcard support.
    fn matches_pattern(pattern: &str, value: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with('*') && pattern.ends_with('*') {
            let middle = &pattern[1..pattern.len() - 1];
            return value.contains(middle);
        }

        if let Some(stripped) = pattern.strip_prefix('*') {
            return value.ends_with(stripped);
        }

        if let Some(stripped) = pattern.strip_suffix('*') {
            return value.starts_with(stripped);
        }

        pattern == value
    }

    /// Path pattern matching with home directory expansion.
    fn matches_path_pattern(pattern: &str, path: &str) -> bool {
        let expanded_pattern = if pattern.starts_with("~/") {
            dirs::home_dir()
                .map(|h| format!("{}/{}", h.display(), &pattern[2..]))
                .unwrap_or_else(|| pattern.to_string())
        } else {
            pattern.to_string()
        };

        Self::matches_pattern(&expanded_pattern, path)
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

/// Network access policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Allow any outbound network connections.
    #[serde(default = "default_true")]
    pub allow_outbound: bool,

    /// Allowed domain patterns (supports wildcards).
    #[serde(default)]
    pub allowed_domains: Vec<String>,

    /// Blocked domain patterns.
    #[serde(default)]
    pub blocked_domains: Vec<String>,

    /// Allow localhost connections.
    #[serde(default = "default_true")]
    pub allow_localhost: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allow_outbound: true,
            allowed_domains: vec![
                "github.com".to_string(),
                "*.github.com".to_string(),
                "crates.io".to_string(),
                "docs.rs".to_string(),
                "api.anthropic.com".to_string(),
            ],
            blocked_domains: vec![],
            allow_localhost: true,
        }
    }
}

/// Filesystem access policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    /// Allowed read path patterns.
    #[serde(default)]
    pub allowed_read_paths: Vec<String>,

    /// Allowed write path patterns.
    #[serde(default)]
    pub allowed_write_paths: Vec<String>,

    /// Blocked path patterns (takes precedence).
    #[serde(default)]
    pub blocked_paths: Vec<String>,

    /// Allow read/write in home directory.
    #[serde(default = "default_true")]
    pub allow_home_access: bool,

    /// Allow read/write in temp directories.
    #[serde(default = "default_true")]
    pub allow_temp_access: bool,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            allowed_read_paths: vec!["~/*".to_string()],
            allowed_write_paths: vec![
                "~/projects/*".to_string(),
                "~/code/*".to_string(),
                "~/repos/*".to_string(),
            ],
            blocked_paths: vec![
                "~/.ssh/*".to_string(),
                "~/.gnupg/*".to_string(),
                "~/.aws/*".to_string(),
                "/etc/*".to_string(),
            ],
            allow_home_access: true,
            allow_temp_access: true,
        }
    }
}

/// Execution policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    /// Allow shell command execution.
    #[serde(default = "default_true")]
    pub allow_shell_commands: bool,

    /// Allowed command patterns.
    #[serde(default)]
    pub allowed_commands: Vec<String>,

    /// Blocked command patterns.
    #[serde(default)]
    pub blocked_commands: Vec<String>,

    /// Allow background/daemon processes.
    #[serde(default)]
    pub allow_background_processes: bool,

    /// Maximum execution time in seconds.
    #[serde(default = "default_max_execution_time")]
    pub max_execution_time_secs: u64,
}

fn default_max_execution_time() -> u64 {
    300 // 5 minutes
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            allow_shell_commands: true,
            allowed_commands: vec![
                "cargo".to_string(),
                "rustc".to_string(),
                "git".to_string(),
                "npm".to_string(),
                "node".to_string(),
                "python*".to_string(),
                "pip*".to_string(),
                "docker".to_string(),
                "kubectl".to_string(),
                "ls".to_string(),
                "cat".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "head".to_string(),
                "tail".to_string(),
            ],
            blocked_commands: vec![
                "rm -rf /*".to_string(),
                "sudo*".to_string(),
                "su".to_string(),
                "chmod 777*".to_string(),
                "curl*|*sh".to_string(),
            ],
            allow_background_processes: false,
            max_execution_time_secs: default_max_execution_time(),
        }
    }
}

/// Trusted identity entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedIdentity {
    /// Identity ID or public key.
    pub id: String,

    /// Human-readable name.
    pub name: Option<String>,

    /// Trust level.
    #[serde(default)]
    pub trust_level: TrustLevel,

    /// When this identity was added.
    pub added_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Whether this identity has been revoked.
    #[serde(default)]
    pub revoked: bool,

    /// Notes about this identity.
    pub notes: Option<String>,
}

/// Trust level for identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// No special trust.
    #[default]
    None,
    /// Basic trust - can read.
    Read,
    /// Elevated trust - can read and write.
    Write,
    /// Full trust - can execute.
    Execute,
    /// Administrative trust - full access.
    Admin,
}

/// Approval requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    /// Require approval for network operations.
    #[serde(default)]
    pub require_approval_for_network: bool,

    /// Require approval for write operations.
    #[serde(default)]
    pub require_approval_for_writes: bool,

    /// Require approval for command execution.
    #[serde(default = "default_true")]
    pub require_approval_for_execution: bool,

    /// Require approval for secret access.
    #[serde(default = "default_true")]
    pub require_approval_for_secrets: bool,

    /// Auto-approve actions from trusted identities.
    #[serde(default)]
    pub auto_approve_trusted: bool,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self {
            require_approval_for_network: false,
            require_approval_for_writes: false,
            require_approval_for_execution: true,
            require_approval_for_secrets: true,
            auto_approve_trusted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = TrustPolicy::default();
        assert!(policy.network.allow_outbound);
        assert!(policy.allows_network_access("github.com"));
    }

    #[test]
    fn test_restrictive_policy() {
        let policy = TrustPolicy::restrictive();
        assert!(!policy.network.allow_outbound);
        assert!(!policy.allows_network_access("github.com"));
        assert!(!policy.execution.allow_shell_commands);
    }

    #[test]
    fn test_permissive_policy() {
        let policy = TrustPolicy::permissive();
        assert!(policy.allows_network_access("any-domain.com"));
        assert!(policy.allows_command("any-command"));
    }

    #[test]
    fn test_domain_blocking() {
        let mut policy = TrustPolicy::default();
        policy.network.blocked_domains.push("evil.com".to_string());

        assert!(!policy.allows_network_access("evil.com"));
        assert!(policy.allows_network_access("github.com"));
    }

    #[test]
    fn test_wildcard_patterns() {
        let policy = TrustPolicy::default();

        // Test wildcard in allowed domains
        assert!(policy.allows_network_access("api.github.com"));
        assert!(policy.allows_network_access("raw.github.com"));
    }

    #[test]
    fn test_command_blocking() {
        let policy = TrustPolicy::default();

        assert!(policy.allows_command("cargo build"));
        assert!(policy.allows_command("git status"));
        assert!(!policy.allows_command("sudo rm -rf /"));
    }

    #[test]
    fn test_toml_roundtrip() {
        let policy = TrustPolicy::default();
        let toml = policy.to_toml().unwrap();
        let loaded = TrustPolicy::from_toml(&toml).unwrap();

        assert_eq!(policy.network.allow_outbound, loaded.network.allow_outbound);
        assert_eq!(policy.version, loaded.version);
    }

    #[test]
    fn test_trusted_identity() {
        let mut policy = TrustPolicy::default();
        policy.trusted_identities.push(TrustedIdentity {
            id: "uid_abc123".to_string(),
            name: Some("Test User".to_string()),
            trust_level: TrustLevel::Execute,
            added_at: Some(chrono::Utc::now()),
            revoked: false,
            notes: None,
        });

        assert!(policy.is_identity_trusted("uid_abc123"));
        assert!(!policy.is_identity_trusted("uid_unknown"));
    }

    #[test]
    fn test_revoked_identity() {
        let mut policy = TrustPolicy::default();
        policy.trusted_identities.push(TrustedIdentity {
            id: "uid_revoked".to_string(),
            name: None,
            trust_level: TrustLevel::Admin,
            added_at: None,
            revoked: true,
            notes: Some("Compromised".to_string()),
        });

        assert!(!policy.is_identity_trusted("uid_revoked"));
    }
}
