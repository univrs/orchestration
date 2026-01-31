//! Automation boundaries configuration.
//!
//! Defines boundaries for automated operations, including what can be done
//! automatically vs. what requires human approval.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Automation level for operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutomationLevel {
    /// No automation - all actions require explicit approval.
    None,
    /// Minimal automation - only safe read operations.
    #[default]
    Minimal,
    /// Standard automation - common development operations.
    Standard,
    /// Extended automation - most operations except destructive ones.
    Extended,
    /// Full automation - all operations (use with caution).
    Full,
}

impl AutomationLevel {
    /// Check if this level allows a given operation category.
    pub fn allows(&self, category: OperationCategory) -> bool {
        match (self, category) {
            // None allows nothing automatic
            (AutomationLevel::None, _) => false,

            // Minimal allows only safe reads
            (AutomationLevel::Minimal, OperationCategory::SafeRead) => true,
            (AutomationLevel::Minimal, _) => false,

            // Standard allows common dev operations
            (AutomationLevel::Standard, OperationCategory::SafeRead) => true,
            (AutomationLevel::Standard, OperationCategory::SafeWrite) => true,
            (AutomationLevel::Standard, OperationCategory::SafeExecute) => true,
            (AutomationLevel::Standard, OperationCategory::NetworkRead) => true,
            (AutomationLevel::Standard, _) => false,

            // Extended allows most operations
            (AutomationLevel::Extended, OperationCategory::Destructive) => false,
            (AutomationLevel::Extended, OperationCategory::SecretAccess) => false,
            (AutomationLevel::Extended, _) => true,

            // Full allows everything
            (AutomationLevel::Full, _) => true,
        }
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            AutomationLevel::None => "No automation - all actions require explicit approval",
            AutomationLevel::Minimal => "Minimal - only safe read operations",
            AutomationLevel::Standard => "Standard - common development operations",
            AutomationLevel::Extended => "Extended - most operations except destructive",
            AutomationLevel::Full => "Full - all operations (use with caution)",
        }
    }
}

/// Categories of operations for automation boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationCategory {
    /// Safe read operations (e.g., reading source files).
    SafeRead,
    /// Safe write operations (e.g., writing to project files).
    SafeWrite,
    /// Safe execution (e.g., cargo build, git status).
    SafeExecute,
    /// Network read operations (e.g., API calls).
    NetworkRead,
    /// Network write operations (e.g., pushing to git).
    NetworkWrite,
    /// System modifications (e.g., installing packages).
    SystemModify,
    /// Destructive operations (e.g., deleting files).
    Destructive,
    /// Secret/credential access.
    SecretAccess,
    /// Background processes.
    Background,
}

impl OperationCategory {
    /// Get all categories.
    pub fn all() -> Vec<Self> {
        vec![
            Self::SafeRead,
            Self::SafeWrite,
            Self::SafeExecute,
            Self::NetworkRead,
            Self::NetworkWrite,
            Self::SystemModify,
            Self::Destructive,
            Self::SecretAccess,
            Self::Background,
        ]
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::SafeRead => "Read files and directories",
            Self::SafeWrite => "Write to project files",
            Self::SafeExecute => "Run safe commands (build, test, lint)",
            Self::NetworkRead => "Make network requests",
            Self::NetworkWrite => "Push changes, publish packages",
            Self::SystemModify => "Install packages, modify system",
            Self::Destructive => "Delete files, reset repositories",
            Self::SecretAccess => "Access secrets and credentials",
            Self::Background => "Run background processes",
        }
    }
}

/// Automation boundary configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationBoundary {
    /// Global automation level.
    #[serde(default)]
    pub level: AutomationLevel,

    /// Per-category overrides.
    #[serde(default)]
    pub category_overrides: HashMap<OperationCategory, bool>,

    /// Operations that always require approval.
    #[serde(default)]
    pub always_require_approval: Vec<String>,

    /// Operations that are always allowed.
    #[serde(default)]
    pub always_allow: Vec<String>,

    /// Maximum consecutive automated operations.
    #[serde(default = "default_max_consecutive")]
    pub max_consecutive_operations: u32,

    /// Cooldown period between automated operations (seconds).
    #[serde(default)]
    pub cooldown_seconds: u32,

    /// Whether to notify on automated operations.
    #[serde(default = "default_true")]
    pub notify_on_automation: bool,

    /// Scope restrictions.
    #[serde(default)]
    pub scope: AutomationScope,
}

fn default_max_consecutive() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

impl Default for AutomationBoundary {
    fn default() -> Self {
        Self {
            level: AutomationLevel::Standard,
            category_overrides: HashMap::new(),
            always_require_approval: vec![
                "git push --force".to_string(),
                "rm -rf".to_string(),
                "DROP TABLE".to_string(),
                "DELETE FROM".to_string(),
            ],
            always_allow: vec![
                "cargo check".to_string(),
                "cargo build".to_string(),
                "cargo test".to_string(),
                "cargo clippy".to_string(),
                "git status".to_string(),
                "git diff".to_string(),
                "git log".to_string(),
            ],
            max_consecutive_operations: default_max_consecutive(),
            cooldown_seconds: 0,
            notify_on_automation: true,
            scope: AutomationScope::default(),
        }
    }
}

impl AutomationBoundary {
    /// Create a restrictive boundary (minimal automation).
    pub fn restrictive() -> Self {
        Self {
            level: AutomationLevel::Minimal,
            category_overrides: HashMap::new(),
            always_require_approval: OperationCategory::all()
                .iter()
                .map(|c| format!("{:?}", c))
                .collect(),
            always_allow: vec![],
            max_consecutive_operations: 1,
            cooldown_seconds: 5,
            notify_on_automation: true,
            scope: AutomationScope::restrictive(),
        }
    }

    /// Create a permissive boundary (extended automation).
    pub fn permissive() -> Self {
        Self {
            level: AutomationLevel::Extended,
            category_overrides: HashMap::new(),
            always_require_approval: vec!["rm -rf /".to_string(), "DROP DATABASE".to_string()],
            always_allow: vec!["*".to_string()],
            max_consecutive_operations: 100,
            cooldown_seconds: 0,
            notify_on_automation: false,
            scope: AutomationScope::permissive(),
        }
    }

    /// Check if an operation is allowed automatically.
    pub fn is_automated(&self, category: OperationCategory, operation: &str) -> AutomationDecision {
        // Check always require approval list
        for pattern in &self.always_require_approval {
            if Self::matches_pattern(pattern, operation) {
                return AutomationDecision::RequiresApproval {
                    reason: format!(
                        "Operation matches always-require-approval pattern: {}",
                        pattern
                    ),
                };
            }
        }

        // Check always allow list
        for pattern in &self.always_allow {
            if Self::matches_pattern(pattern, operation) {
                return AutomationDecision::Allowed;
            }
        }

        // Check category overrides
        if let Some(&allowed) = self.category_overrides.get(&category) {
            if allowed {
                return AutomationDecision::Allowed;
            } else {
                return AutomationDecision::RequiresApproval {
                    reason: format!("Category {:?} is disabled by override", category),
                };
            }
        }

        // Check automation level
        if self.level.allows(category) {
            AutomationDecision::Allowed
        } else {
            AutomationDecision::RequiresApproval {
                reason: format!(
                    "Category {:?} not allowed at automation level {:?}",
                    category, self.level
                ),
            }
        }
    }

    /// Simple pattern matching.
    fn matches_pattern(pattern: &str, value: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with('*') && pattern.ends_with('*') {
            let middle = &pattern[1..pattern.len() - 1];
            return value.contains(middle);
        }

        if pattern.starts_with('*') {
            return value.ends_with(&pattern[1..]);
        }

        if pattern.ends_with('*') {
            return value.starts_with(&pattern[..pattern.len() - 1]);
        }

        value.contains(pattern)
    }
}

/// Result of automation decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationDecision {
    /// Operation is allowed automatically.
    Allowed,
    /// Operation requires human approval.
    RequiresApproval { reason: String },
    /// Operation is blocked entirely.
    Blocked { reason: String },
}

impl AutomationDecision {
    /// Check if the operation is allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, AutomationDecision::Allowed)
    }

    /// Check if approval is required.
    pub fn requires_approval(&self) -> bool {
        matches!(self, AutomationDecision::RequiresApproval { .. })
    }

    /// Check if the operation is blocked.
    pub fn is_blocked(&self) -> bool {
        matches!(self, AutomationDecision::Blocked { .. })
    }
}

/// Scope restrictions for automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationScope {
    /// Allowed project paths.
    #[serde(default)]
    pub allowed_paths: Vec<String>,

    /// Allowed git repositories.
    #[serde(default)]
    pub allowed_repos: Vec<String>,

    /// Allowed file extensions for writes.
    #[serde(default)]
    pub allowed_write_extensions: Vec<String>,

    /// Maximum file size for automated writes (bytes).
    #[serde(default = "default_max_file_size")]
    pub max_write_file_size: u64,

    /// Maximum number of files to modify in one operation.
    #[serde(default = "default_max_files")]
    pub max_files_per_operation: u32,
}

fn default_max_file_size() -> u64 {
    1024 * 1024 // 1 MB
}

fn default_max_files() -> u32 {
    20
}

impl Default for AutomationScope {
    fn default() -> Self {
        Self {
            allowed_paths: vec!["~/projects/*".to_string(), "~/code/*".to_string()],
            allowed_repos: vec!["*".to_string()],
            allowed_write_extensions: vec![
                ".rs".to_string(),
                ".toml".to_string(),
                ".json".to_string(),
                ".yaml".to_string(),
                ".yml".to_string(),
                ".md".to_string(),
                ".txt".to_string(),
                ".ts".to_string(),
                ".js".to_string(),
                ".py".to_string(),
                ".go".to_string(),
            ],
            max_write_file_size: default_max_file_size(),
            max_files_per_operation: default_max_files(),
        }
    }
}

impl AutomationScope {
    /// Create restrictive scope.
    pub fn restrictive() -> Self {
        Self {
            allowed_paths: vec![],
            allowed_repos: vec![],
            allowed_write_extensions: vec![".md".to_string(), ".txt".to_string()],
            max_write_file_size: 10 * 1024, // 10 KB
            max_files_per_operation: 1,
        }
    }

    /// Create permissive scope.
    pub fn permissive() -> Self {
        Self {
            allowed_paths: vec!["*".to_string()],
            allowed_repos: vec!["*".to_string()],
            allowed_write_extensions: vec!["*".to_string()],
            max_write_file_size: 100 * 1024 * 1024, // 100 MB
            max_files_per_operation: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_level_allows() {
        assert!(!AutomationLevel::None.allows(OperationCategory::SafeRead));
        assert!(AutomationLevel::Minimal.allows(OperationCategory::SafeRead));
        assert!(!AutomationLevel::Minimal.allows(OperationCategory::SafeWrite));
        assert!(AutomationLevel::Standard.allows(OperationCategory::SafeWrite));
        assert!(!AutomationLevel::Standard.allows(OperationCategory::Destructive));
        assert!(AutomationLevel::Full.allows(OperationCategory::Destructive));
    }

    #[test]
    fn test_default_boundary() {
        let boundary = AutomationBoundary::default();
        assert_eq!(boundary.level, AutomationLevel::Standard);

        let decision = boundary.is_automated(OperationCategory::SafeRead, "cat file.txt");
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_always_require_approval() {
        let boundary = AutomationBoundary::default();

        let decision = boundary.is_automated(OperationCategory::Destructive, "rm -rf /tmp");
        assert!(decision.requires_approval());
    }

    #[test]
    fn test_always_allow() {
        let boundary = AutomationBoundary::default();

        let decision = boundary.is_automated(OperationCategory::SafeExecute, "cargo test");
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_category_override() {
        let mut boundary = AutomationBoundary::default();
        boundary
            .category_overrides
            .insert(OperationCategory::SafeExecute, false);

        let decision = boundary.is_automated(OperationCategory::SafeExecute, "cargo build");
        // Note: "cargo build" is in always_allow, so it should still be allowed
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_restrictive_boundary() {
        let boundary = AutomationBoundary::restrictive();
        assert_eq!(boundary.level, AutomationLevel::Minimal);
        assert_eq!(boundary.max_consecutive_operations, 1);
    }

    #[test]
    fn test_permissive_boundary() {
        let boundary = AutomationBoundary::permissive();
        assert_eq!(boundary.level, AutomationLevel::Extended);
    }
}
