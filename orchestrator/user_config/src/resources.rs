//! Resource limits configuration.
//!
//! Defines limits on resource consumption for automated operations.

use serde::{Deserialize, Serialize};

/// Resource limits for automated operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU limits.
    #[serde(default)]
    pub cpu: CpuLimits,

    /// Memory limits.
    #[serde(default)]
    pub memory: MemoryLimits,

    /// Disk limits.
    #[serde(default)]
    pub disk: DiskLimits,

    /// Network limits.
    #[serde(default)]
    pub network: NetworkLimits,

    /// Time limits.
    #[serde(default)]
    pub time: TimeLimits,

    /// Process limits.
    #[serde(default)]
    pub processes: ProcessLimits,

    /// API rate limits.
    #[serde(default)]
    pub api: ApiLimits,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu: CpuLimits::default(),
            memory: MemoryLimits::default(),
            disk: DiskLimits::default(),
            network: NetworkLimits::default(),
            time: TimeLimits::default(),
            processes: ProcessLimits::default(),
            api: ApiLimits::default(),
        }
    }
}

impl ResourceLimits {
    /// Create restrictive resource limits.
    pub fn restrictive() -> Self {
        Self {
            cpu: CpuLimits {
                max_cores: 1.0,
                max_usage_percent: 25,
            },
            memory: MemoryLimits {
                max_mb: 512,
                warn_mb: 256,
            },
            disk: DiskLimits {
                max_write_mb: 100,
                max_read_mb: 1024,
                max_files_open: 50,
                max_file_size_mb: 10,
            },
            network: NetworkLimits {
                max_bandwidth_kbps: 1024,
                max_connections: 5,
                max_requests_per_minute: 30,
            },
            time: TimeLimits {
                max_operation_seconds: 60,
                max_session_minutes: 30,
                max_idle_minutes: 5,
            },
            processes: ProcessLimits {
                max_concurrent: 2,
                max_background: 0,
                max_child_processes: 5,
            },
            api: ApiLimits {
                max_tokens_per_request: 4096,
                max_requests_per_hour: 50,
                max_cost_per_hour_cents: 100,
            },
        }
    }

    /// Create permissive resource limits.
    pub fn permissive() -> Self {
        Self {
            cpu: CpuLimits {
                max_cores: 8.0,
                max_usage_percent: 100,
            },
            memory: MemoryLimits {
                max_mb: 16384,
                warn_mb: 8192,
            },
            disk: DiskLimits {
                max_write_mb: 10240,
                max_read_mb: 102400,
                max_files_open: 1000,
                max_file_size_mb: 1024,
            },
            network: NetworkLimits {
                max_bandwidth_kbps: 0, // Unlimited
                max_connections: 100,
                max_requests_per_minute: 1000,
            },
            time: TimeLimits {
                max_operation_seconds: 3600,
                max_session_minutes: 480,
                max_idle_minutes: 60,
            },
            processes: ProcessLimits {
                max_concurrent: 20,
                max_background: 10,
                max_child_processes: 100,
            },
            api: ApiLimits {
                max_tokens_per_request: 200000,
                max_requests_per_hour: 1000,
                max_cost_per_hour_cents: 10000,
            },
        }
    }

    /// Check if a resource usage is within limits.
    pub fn check_cpu(&self, cores: f32) -> ResourceCheck {
        if cores <= self.cpu.max_cores {
            ResourceCheck::Ok
        } else {
            ResourceCheck::Exceeded {
                resource: "CPU cores".to_string(),
                limit: self.cpu.max_cores.to_string(),
                actual: cores.to_string(),
            }
        }
    }

    /// Check memory usage.
    pub fn check_memory(&self, mb: u64) -> ResourceCheck {
        if mb <= self.memory.max_mb {
            if mb >= self.memory.warn_mb {
                ResourceCheck::Warning {
                    resource: "Memory".to_string(),
                    message: format!(
                        "Memory usage ({} MB) approaching limit ({} MB)",
                        mb, self.memory.max_mb
                    ),
                }
            } else {
                ResourceCheck::Ok
            }
        } else {
            ResourceCheck::Exceeded {
                resource: "Memory".to_string(),
                limit: format!("{} MB", self.memory.max_mb),
                actual: format!("{} MB", mb),
            }
        }
    }

    /// Check disk write.
    pub fn check_disk_write(&self, mb: u64) -> ResourceCheck {
        if mb <= self.disk.max_write_mb {
            ResourceCheck::Ok
        } else {
            ResourceCheck::Exceeded {
                resource: "Disk write".to_string(),
                limit: format!("{} MB", self.disk.max_write_mb),
                actual: format!("{} MB", mb),
            }
        }
    }

    /// Check file size.
    pub fn check_file_size(&self, mb: u64) -> ResourceCheck {
        if mb <= self.disk.max_file_size_mb {
            ResourceCheck::Ok
        } else {
            ResourceCheck::Exceeded {
                resource: "File size".to_string(),
                limit: format!("{} MB", self.disk.max_file_size_mb),
                actual: format!("{} MB", mb),
            }
        }
    }

    /// Check operation time.
    pub fn check_operation_time(&self, seconds: u64) -> ResourceCheck {
        if seconds <= self.time.max_operation_seconds {
            ResourceCheck::Ok
        } else {
            ResourceCheck::Exceeded {
                resource: "Operation time".to_string(),
                limit: format!("{} seconds", self.time.max_operation_seconds),
                actual: format!("{} seconds", seconds),
            }
        }
    }

    /// Check concurrent processes.
    pub fn check_processes(&self, count: u32) -> ResourceCheck {
        if count <= self.processes.max_concurrent {
            ResourceCheck::Ok
        } else {
            ResourceCheck::Exceeded {
                resource: "Concurrent processes".to_string(),
                limit: self.processes.max_concurrent.to_string(),
                actual: count.to_string(),
            }
        }
    }
}

/// CPU resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuLimits {
    /// Maximum CPU cores to use.
    #[serde(default = "default_max_cores")]
    pub max_cores: f32,

    /// Maximum CPU usage percentage.
    #[serde(default = "default_max_cpu_percent")]
    pub max_usage_percent: u32,
}

fn default_max_cores() -> f32 {
    4.0
}

fn default_max_cpu_percent() -> u32 {
    80
}

impl Default for CpuLimits {
    fn default() -> Self {
        Self {
            max_cores: default_max_cores(),
            max_usage_percent: default_max_cpu_percent(),
        }
    }
}

/// Memory resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLimits {
    /// Maximum memory in MB.
    #[serde(default = "default_max_memory")]
    pub max_mb: u64,

    /// Warning threshold in MB.
    #[serde(default = "default_warn_memory")]
    pub warn_mb: u64,
}

fn default_max_memory() -> u64 {
    4096
}

fn default_warn_memory() -> u64 {
    3072
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_mb: default_max_memory(),
            warn_mb: default_warn_memory(),
        }
    }
}

/// Disk resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskLimits {
    /// Maximum disk write in MB per session.
    #[serde(default = "default_max_write")]
    pub max_write_mb: u64,

    /// Maximum disk read in MB per session.
    #[serde(default = "default_max_read")]
    pub max_read_mb: u64,

    /// Maximum open files.
    #[serde(default = "default_max_files")]
    pub max_files_open: u32,

    /// Maximum single file size in MB.
    #[serde(default = "default_max_file_size")]
    pub max_file_size_mb: u64,
}

fn default_max_write() -> u64 {
    1024
}

fn default_max_read() -> u64 {
    10240
}

fn default_max_files() -> u32 {
    100
}

fn default_max_file_size() -> u64 {
    100
}

impl Default for DiskLimits {
    fn default() -> Self {
        Self {
            max_write_mb: default_max_write(),
            max_read_mb: default_max_read(),
            max_files_open: default_max_files(),
            max_file_size_mb: default_max_file_size(),
        }
    }
}

/// Network resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLimits {
    /// Maximum bandwidth in kbps (0 = unlimited).
    #[serde(default)]
    pub max_bandwidth_kbps: u64,

    /// Maximum concurrent connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Maximum requests per minute.
    #[serde(default = "default_max_requests")]
    pub max_requests_per_minute: u32,
}

fn default_max_connections() -> u32 {
    20
}

fn default_max_requests() -> u32 {
    100
}

impl Default for NetworkLimits {
    fn default() -> Self {
        Self {
            max_bandwidth_kbps: 0, // Unlimited
            max_connections: default_max_connections(),
            max_requests_per_minute: default_max_requests(),
        }
    }
}

/// Time resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLimits {
    /// Maximum time for a single operation in seconds.
    #[serde(default = "default_max_operation_time")]
    pub max_operation_seconds: u64,

    /// Maximum session duration in minutes.
    #[serde(default = "default_max_session")]
    pub max_session_minutes: u64,

    /// Maximum idle time in minutes.
    #[serde(default = "default_max_idle")]
    pub max_idle_minutes: u64,
}

fn default_max_operation_time() -> u64 {
    300 // 5 minutes
}

fn default_max_session() -> u64 {
    120 // 2 hours
}

fn default_max_idle() -> u64 {
    15
}

impl Default for TimeLimits {
    fn default() -> Self {
        Self {
            max_operation_seconds: default_max_operation_time(),
            max_session_minutes: default_max_session(),
            max_idle_minutes: default_max_idle(),
        }
    }
}

/// Process limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessLimits {
    /// Maximum concurrent processes.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,

    /// Maximum background processes.
    #[serde(default = "default_max_background")]
    pub max_background: u32,

    /// Maximum child processes per operation.
    #[serde(default = "default_max_child")]
    pub max_child_processes: u32,
}

fn default_max_concurrent() -> u32 {
    5
}

fn default_max_background() -> u32 {
    2
}

fn default_max_child() -> u32 {
    20
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            max_concurrent: default_max_concurrent(),
            max_background: default_max_background(),
            max_child_processes: default_max_child(),
        }
    }
}

/// API rate limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLimits {
    /// Maximum tokens per request.
    #[serde(default = "default_max_tokens")]
    pub max_tokens_per_request: u64,

    /// Maximum API requests per hour.
    #[serde(default = "default_max_api_requests")]
    pub max_requests_per_hour: u64,

    /// Maximum cost per hour in cents.
    #[serde(default = "default_max_cost")]
    pub max_cost_per_hour_cents: u64,
}

fn default_max_tokens() -> u64 {
    100000
}

fn default_max_api_requests() -> u64 {
    200
}

fn default_max_cost() -> u64 {
    500 // $5
}

impl Default for ApiLimits {
    fn default() -> Self {
        Self {
            max_tokens_per_request: default_max_tokens(),
            max_requests_per_hour: default_max_api_requests(),
            max_cost_per_hour_cents: default_max_cost(),
        }
    }
}

/// Result of a resource check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCheck {
    /// Within limits.
    Ok,
    /// Approaching limit.
    Warning { resource: String, message: String },
    /// Exceeded limit.
    Exceeded {
        resource: String,
        limit: String,
        actual: String,
    },
}

impl ResourceCheck {
    /// Check if the resource is within limits.
    pub fn is_ok(&self) -> bool {
        matches!(self, ResourceCheck::Ok)
    }

    /// Check if there's a warning.
    pub fn is_warning(&self) -> bool {
        matches!(self, ResourceCheck::Warning { .. })
    }

    /// Check if the limit was exceeded.
    pub fn is_exceeded(&self) -> bool {
        matches!(self, ResourceCheck::Exceeded { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu.max_cores, 4.0);
        assert_eq!(limits.memory.max_mb, 4096);
    }

    #[test]
    fn test_restrictive_limits() {
        let limits = ResourceLimits::restrictive();
        assert_eq!(limits.cpu.max_cores, 1.0);
        assert_eq!(limits.processes.max_background, 0);
    }

    #[test]
    fn test_permissive_limits() {
        let limits = ResourceLimits::permissive();
        assert_eq!(limits.cpu.max_cores, 8.0);
        assert_eq!(limits.processes.max_concurrent, 20);
    }

    #[test]
    fn test_check_cpu_ok() {
        let limits = ResourceLimits::default();
        assert!(limits.check_cpu(2.0).is_ok());
    }

    #[test]
    fn test_check_cpu_exceeded() {
        let limits = ResourceLimits::default();
        assert!(limits.check_cpu(10.0).is_exceeded());
    }

    #[test]
    fn test_check_memory_warning() {
        let limits = ResourceLimits::default();
        let check = limits.check_memory(3500);
        assert!(check.is_warning());
    }

    #[test]
    fn test_check_memory_exceeded() {
        let limits = ResourceLimits::default();
        assert!(limits.check_memory(5000).is_exceeded());
    }

    #[test]
    fn test_check_operation_time() {
        let limits = ResourceLimits::default();
        assert!(limits.check_operation_time(100).is_ok());
        assert!(limits.check_operation_time(1000).is_exceeded());
    }

    #[test]
    fn test_serialization() {
        let limits = ResourceLimits::default();
        let toml = toml::to_string(&limits).unwrap();
        let loaded: ResourceLimits = toml::from_str(&toml).unwrap();
        assert_eq!(limits.cpu.max_cores, loaded.cpu.max_cores);
    }
}
