//! User Configuration Crate for AI-Native Orchestrator
//!
//! This crate provides:
//! - Ed25519 identity generation and management
//! - Age encryption for secrets
//! - Trust policy configuration via TOML
//! - Automation boundaries and resource limits
//! - XDG-compliant configuration paths
//!
//! # Configuration Location
//!
//! All configuration is stored under `~/.config/univrs/` following XDG standards:
//! - `~/.config/univrs/identity.toml` - User identity and keys
//! - `~/.config/univrs/trust_policy.toml` - Trust and automation policies
//! - `~/.config/univrs/secrets/` - Encrypted secrets directory
//!
//! # Example
//!
//! ```no_run
//! use user_config::{UserConfig, Identity};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Load or create user configuration
//!     let config = UserConfig::load_or_create().await?;
//!
//!     // Access identity
//!     let identity = config.identity();
//!     println!("User ID: {}", identity.id());
//!
//!     // Check trust policy
//!     if config.trust_policy().allows_network_access("github.com") {
//!         println!("GitHub access allowed");
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod automation;
pub mod config;
pub mod encryption;
mod error;
pub mod identity;
pub mod paths;
pub mod resources;
pub mod trust_policy;

pub use automation::{AutomationBoundary, AutomationLevel};
pub use config::UserConfig;
pub use encryption::SecretStore;
pub use error::{ConfigError, Result};
pub use identity::Identity;
pub use paths::ConfigPaths;
pub use resources::ResourceLimits;
pub use trust_policy::TrustPolicy;

/// Application name used for XDG paths
pub const APP_NAME: &str = "univrs";

/// Current configuration schema version
pub const CONFIG_VERSION: &str = "1.0";
