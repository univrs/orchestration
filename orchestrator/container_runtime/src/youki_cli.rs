//! CLI-based Youki container runtime.
//!
//! This module provides a container runtime implementation that uses the
//! `youki` CLI binary to manage OCI containers.
//!
//! # Requirements
//!
//! - `youki` binary must be installed and in PATH
//! - Linux with cgroups v2
//! - Root privileges (or appropriate capabilities)
//!
//! # Log Collection
//!
//! Container stdout/stderr is captured to log files stored at:
//! `{state_root}/{container_id}/container.log`
//!
//! The log file contains both stdout and stderr interleaved with timestamps.
//! Use `get_logs()` or `stream_logs()` to access container logs.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use container_runtime_interface::{ContainerRuntime, ContainerStatus, CreateContainerOptions};
use orchestrator_shared_types::{ContainerConfig, ContainerId, NodeId, OrchestrationError, Result};

use crate::image::ImageManager;
use crate::oci_bundle::OciBundleBuilder;

/// Errors specific to Youki CLI operations.
#[derive(Debug, thiserror::Error)]
pub enum YoukiCliError {
    #[error("Youki binary not found: {0}")]
    BinaryNotFound(String),

    #[error("Youki command failed: {command} - {message}")]
    CommandFailed { command: String, message: String },

    #[error("Container not found: {0}")]
    ContainerNotFound(String),

    #[error("Invalid state output: {0}")]
    InvalidState(String),

    #[error("Image error: {0}")]
    Image(#[from] crate::image::ImageError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Log error: {0}")]
    LogError(String),
}

/// A single log entry from a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp of the log entry (RFC3339 format)
    pub timestamp: String,
    /// Stream source: "stdout" or "stderr"
    pub stream: String,
    /// The log message
    pub message: String,
}

/// Options for retrieving container logs.
#[derive(Debug, Clone, Default)]
pub struct LogOptions {
    /// Return only the last N lines
    pub tail: Option<usize>,
    /// Include timestamps in output
    pub timestamps: bool,
    /// Only return logs since this timestamp (RFC3339)
    pub since: Option<String>,
    /// Only return logs until this timestamp (RFC3339)
    pub until: Option<String>,
    /// Follow log output (like tail -f)
    pub follow: bool,
}

/// Log stream receiver for follow mode.
pub type LogReceiver = broadcast::Receiver<LogEntry>;

/// Internal handle for active log streams.
struct LogStreamHandle {
    sender: broadcast::Sender<LogEntry>,
    /// Task handle for the log watcher
    _watcher: tokio::task::JoinHandle<()>,
}

impl From<YoukiCliError> for OrchestrationError {
    fn from(err: YoukiCliError) -> Self {
        OrchestrationError::RuntimeError(err.to_string())
    }
}

/// Container state from `youki state` command.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct YoukiState {
    #[serde(rename = "ociVersion")]
    pub oci_version: String,
    pub id: String,
    pub status: String,
    pub pid: Option<i32>,
    pub bundle: String,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    #[serde(rename = "created", default)]
    pub created: Option<String>,
}

/// Internal container state tracking.
#[derive(Debug, Clone)]
pub struct ContainerState {
    pub id: ContainerId,
    pub node_id: NodeId,
    pub bundle_path: PathBuf,
    pub status: String,
    pub pid: Option<i32>,
}

/// Configuration for YoukiCliRuntime.
#[derive(Debug, Clone)]
pub struct YoukiCliConfig {
    /// Path to youki binary (default: "youki")
    pub youki_binary: PathBuf,
    /// Base directory for container bundles
    pub bundle_root: PathBuf,
    /// Root directory for youki state
    pub state_root: PathBuf,
    /// Timeout for commands (default: 30s)
    pub command_timeout: Duration,
    /// Timeout before SIGKILL (default: 10s)
    pub stop_timeout: Duration,
}

impl Default for YoukiCliConfig {
    fn default() -> Self {
        Self {
            youki_binary: PathBuf::from("youki"),
            bundle_root: PathBuf::from("/var/lib/orchestrator/bundles"),
            state_root: PathBuf::from("/run/youki"),
            command_timeout: Duration::from_secs(30),
            stop_timeout: Duration::from_secs(10),
        }
    }
}

/// CLI-based Youki container runtime.
pub struct YoukiCliRuntime {
    config: YoukiCliConfig,
    image_manager: ImageManager,
    containers: Arc<RwLock<HashMap<String, ContainerState>>>,
    containers_by_node: Arc<RwLock<HashMap<NodeId, Vec<ContainerId>>>>,
    /// Active log streams for follow mode
    log_streams: Arc<RwLock<HashMap<String, LogStreamHandle>>>,
}

impl YoukiCliRuntime {
    /// Create a new YoukiCliRuntime with default configuration.
    pub async fn new() -> std::result::Result<Self, YoukiCliError> {
        Self::with_config(YoukiCliConfig::default()).await
    }

    /// Create with custom configuration.
    pub async fn with_config(config: YoukiCliConfig) -> std::result::Result<Self, YoukiCliError> {
        // Verify youki binary exists
        Self::verify_binary(&config.youki_binary).await?;

        // Create directories
        tokio::fs::create_dir_all(&config.bundle_root).await?;
        tokio::fs::create_dir_all(&config.state_root).await?;

        // Initialize image manager
        let image_cache = config.bundle_root.parent()
            .unwrap_or(Path::new("/var/lib/orchestrator"))
            .join("images");
        tokio::fs::create_dir_all(&image_cache).await?;
        let image_manager = ImageManager::new(&image_cache)?;

        info!("YoukiCliRuntime initialized with binary: {:?}", config.youki_binary);

        Ok(Self {
            config,
            image_manager,
            containers: Arc::new(RwLock::new(HashMap::new())),
            containers_by_node: Arc::new(RwLock::new(HashMap::new())),
            log_streams: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Verify youki binary exists and is executable.
    async fn verify_binary(binary: &Path) -> std::result::Result<(), YoukiCliError> {
        let output = Command::new(binary)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| YoukiCliError::BinaryNotFound(format!("{:?}: {}", binary, e)))?;

        if !output.status.success() {
            return Err(YoukiCliError::BinaryNotFound(format!(
                "{:?} returned non-zero exit code",
                binary
            )));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        info!("Youki version: {}", version.trim());
        Ok(())
    }

    /// Get bundle path for a container.
    fn bundle_path(&self, node_id: &NodeId, container_id: &str) -> PathBuf {
        self.config.bundle_root
            .join(node_id.to_string())
            .join(container_id)
    }

    // ==================== Youki CLI Helper Methods ====================

    /// Execute youki command with timeout.
    async fn exec_youki(&self, args: &[&str]) -> std::result::Result<std::process::Output, YoukiCliError> {
        let cmd_str = format!("youki {}", args.join(" "));
        debug!("Executing: {}", cmd_str);

        let result = tokio::time::timeout(
            self.config.command_timeout,
            Command::new(&self.config.youki_binary)
                .args(args)
                .arg("--root")
                .arg(&self.config.state_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| YoukiCliError::Timeout(cmd_str.clone()))?
        .map_err(YoukiCliError::Io)?;

        Ok(result)
    }

    /// youki create <id> --bundle <path>
    pub async fn youki_create(&self, id: &str, bundle_path: &Path) -> std::result::Result<(), YoukiCliError> {
        let bundle_str = bundle_path.to_string_lossy();
        let output = self.exec_youki(&["create", id, "--bundle", &bundle_str]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(YoukiCliError::CommandFailed {
                command: "create".to_string(),
                message: stderr.to_string(),
            });
        }

        debug!("Container {} created", id);
        Ok(())
    }

    /// youki start <id>
    pub async fn youki_start(&self, id: &str) -> std::result::Result<(), YoukiCliError> {
        let output = self.exec_youki(&["start", id]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(YoukiCliError::CommandFailed {
                command: "start".to_string(),
                message: stderr.to_string(),
            });
        }

        debug!("Container {} started", id);
        Ok(())
    }

    /// youki kill <id> <signal>
    pub async fn youki_kill(&self, id: &str, signal: &str) -> std::result::Result<(), YoukiCliError> {
        let output = self.exec_youki(&["kill", id, signal]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Don't error if already stopped
            if !stderr.contains("not running") && !stderr.contains("no such process") {
                return Err(YoukiCliError::CommandFailed {
                    command: format!("kill {}", signal),
                    message: stderr.to_string(),
                });
            }
        }

        debug!("Signal {} sent to container {}", signal, id);
        Ok(())
    }

    /// youki delete <id> [--force]
    pub async fn youki_delete(&self, id: &str, force: bool) -> std::result::Result<(), YoukiCliError> {
        let args = if force {
            vec!["delete", "--force", id]
        } else {
            vec!["delete", id]
        };

        let output = self.exec_youki(&args).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("not exist") && !stderr.contains("not found") {
                return Err(YoukiCliError::CommandFailed {
                    command: "delete".to_string(),
                    message: stderr.to_string(),
                });
            }
        }

        debug!("Container {} deleted", id);
        Ok(())
    }

    /// youki state <id> -> YoukiState
    pub async fn youki_state(&self, id: &str) -> std::result::Result<YoukiState, YoukiCliError> {
        let output = self.exec_youki(&["state", id]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not exist") || stderr.contains("not found") {
                return Err(YoukiCliError::ContainerNotFound(id.to_string()));
            }
            return Err(YoukiCliError::CommandFailed {
                command: "state".to_string(),
                message: stderr.to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let state: YoukiState = serde_json::from_str(&stdout)
            .map_err(|e| YoukiCliError::InvalidState(e.to_string()))?;

        Ok(state)
    }

    // ==================== Log Management Methods ====================

    /// Get log directory for a container.
    fn log_dir(&self, container_id: &str) -> PathBuf {
        self.config.state_root.join(container_id)
    }

    /// Get log file path for a container.
    fn log_path(&self, container_id: &str) -> PathBuf {
        self.log_dir(container_id).join("container.log")
    }

    /// Initialize log capture for a container.
    /// Creates log directory and empty log file.
    async fn init_log_capture(&self, container_id: &str) -> std::result::Result<PathBuf, YoukiCliError> {
        let log_dir = self.log_dir(container_id);
        tokio::fs::create_dir_all(&log_dir).await?;

        let log_path = self.log_path(container_id);

        // Create empty log file if it doesn't exist
        if !log_path.exists() {
            tokio::fs::File::create(&log_path).await?;
        }

        debug!("Log capture initialized at {:?}", log_path);
        Ok(log_path)
    }

    /// Get container logs from state_root (simple version for backward compatibility).
    pub async fn get_logs(&self, container_id: &str, tail: Option<usize>) -> std::result::Result<String, YoukiCliError> {
        self.get_logs_with_options(container_id, &LogOptions {
            tail,
            ..Default::default()
        }).await
    }

    /// Get container logs with full options support.
    pub async fn get_logs_with_options(
        &self,
        container_id: &str,
        options: &LogOptions,
    ) -> std::result::Result<String, YoukiCliError> {
        let log_path = self.log_path(container_id);

        if !log_path.exists() {
            return Ok(String::new());
        }

        let content = tokio::fs::read_to_string(&log_path).await?;
        let mut lines: Vec<&str> = content.lines().collect();

        // Apply tail filter
        if let Some(n) = options.tail {
            let start = lines.len().saturating_sub(n);
            lines = lines[start..].to_vec();
        }

        // Apply since/until filters if timestamps are present in log lines
        if options.since.is_some() || options.until.is_some() {
            lines = lines.into_iter().filter(|line| {
                // Try to extract timestamp from line (format: "TIMESTAMP STREAM MESSAGE")
                if let Some(ts_str) = line.split_whitespace().next() {
                    if let Some(ref since) = options.since {
                        if ts_str < since.as_str() {
                            return false;
                        }
                    }
                    if let Some(ref until) = options.until {
                        if ts_str > until.as_str() {
                            return false;
                        }
                    }
                }
                true
            }).collect();
        }

        // Strip timestamps if not requested
        if !options.timestamps {
            lines = lines.iter().map(|line| {
                // Try to strip timestamp prefix (format: "TIMESTAMP STREAM MESSAGE")
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() >= 3 {
                    parts[2] // Return just the message
                } else {
                    line
                }
            }).collect();
        }

        Ok(lines.join("\n"))
    }

    /// Get structured log entries.
    pub async fn get_log_entries(
        &self,
        container_id: &str,
        options: &LogOptions,
    ) -> std::result::Result<Vec<LogEntry>, YoukiCliError> {
        let log_path = self.log_path(container_id);

        if !log_path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&log_path).await?;
        let mut entries: Vec<LogEntry> = content
            .lines()
            .filter_map(|line| Self::parse_log_line(line))
            .collect();

        // Apply tail filter
        if let Some(n) = options.tail {
            let start = entries.len().saturating_sub(n);
            entries = entries[start..].to_vec();
        }

        // Apply since/until filters
        if let Some(ref since) = options.since {
            entries.retain(|e| e.timestamp.as_str() >= since.as_str());
        }
        if let Some(ref until) = options.until {
            entries.retain(|e| e.timestamp.as_str() <= until.as_str());
        }

        Ok(entries)
    }

    /// Parse a single log line into a LogEntry.
    fn parse_log_line(line: &str) -> Option<LogEntry> {
        if line.is_empty() {
            return None;
        }

        // Expected format: "TIMESTAMP STREAM MESSAGE"
        // e.g., "2024-01-15T10:30:00.123Z stdout Hello world"
        let parts: Vec<&str> = line.splitn(3, ' ').collect();

        // Check if first part looks like a timestamp (contains 'T' for RFC3339)
        let has_timestamp = parts.first()
            .map(|p| p.contains('T') && p.len() > 10)
            .unwrap_or(false);

        if has_timestamp && parts.len() >= 3 {
            // Full format: TIMESTAMP STREAM MESSAGE
            Some(LogEntry {
                timestamp: parts[0].to_string(),
                stream: parts[1].to_string(),
                message: parts[2].to_string(),
            })
        } else if has_timestamp && parts.len() == 2 {
            // Format: TIMESTAMP MESSAGE (no stream)
            Some(LogEntry {
                timestamp: parts[0].to_string(),
                stream: "stdout".to_string(),
                message: parts[1].to_string(),
            })
        } else {
            // Fallback for lines without timestamp - use current time
            Some(LogEntry {
                timestamp: Utc::now().to_rfc3339(),
                stream: "stdout".to_string(),
                message: line.to_string(),
            })
        }
    }

    /// Start streaming logs for a container (follow mode).
    /// Returns a receiver that will receive new log entries as they appear.
    pub async fn stream_logs(
        &self,
        container_id: &str,
    ) -> std::result::Result<LogReceiver, YoukiCliError> {
        let log_path = self.log_path(container_id);

        // Check if already streaming
        {
            let streams = self.log_streams.read().await;
            if let Some(handle) = streams.get(container_id) {
                return Ok(handle.sender.subscribe());
            }
        }

        // Create broadcast channel
        let (sender, receiver) = broadcast::channel(1024);
        let sender_clone = sender.clone();
        let container_id_owned = container_id.to_string();

        // Spawn log watcher task
        let watcher = tokio::spawn(async move {
            if let Err(e) = Self::watch_log_file(log_path, sender_clone).await {
                error!("Log watcher error for {}: {}", container_id_owned, e);
            }
        });

        // Store handle
        let handle = LogStreamHandle {
            sender: sender.clone(),
            _watcher: watcher,
        };

        self.log_streams.write().await.insert(container_id.to_string(), handle);

        Ok(receiver)
    }

    /// Internal log file watcher that sends entries to the broadcast channel.
    async fn watch_log_file(
        log_path: PathBuf,
        sender: broadcast::Sender<LogEntry>,
    ) -> std::result::Result<(), YoukiCliError> {
        // Wait for file to exist
        let mut retries = 0;
        while !log_path.exists() && retries < 30 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            retries += 1;
        }

        if !log_path.exists() {
            return Err(YoukiCliError::LogError(format!(
                "Log file not found: {:?}", log_path
            )));
        }

        let file = tokio::fs::File::open(&log_path).await?;
        let mut reader = BufReader::new(file);

        // Seek to end to only stream new entries
        reader.seek(SeekFrom::End(0)).await?;

        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // No new data, wait and retry
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // Check if channel is closed
                    if sender.receiver_count() == 0 {
                        debug!("No more receivers, stopping log watcher");
                        break;
                    }
                }
                Ok(_) => {
                    if let Some(entry) = Self::parse_log_line(line.trim()) {
                        // Send to channel, ignore errors (no receivers)
                        let _ = sender.send(entry);
                    }
                }
                Err(e) => {
                    error!("Error reading log file: {}", e);
                    return Err(YoukiCliError::Io(e));
                }
            }
        }

        Ok(())
    }

    /// Stop streaming logs for a container.
    pub async fn stop_log_stream(&self, container_id: &str) {
        let mut streams = self.log_streams.write().await;
        if let Some(handle) = streams.remove(container_id) {
            // Abort the watcher task
            handle._watcher.abort();
            debug!("Stopped log stream for {}", container_id);
        }
    }

    /// Write a log entry to the container's log file.
    pub async fn write_log(
        &self,
        container_id: &str,
        stream: &str,
        message: &str,
    ) -> std::result::Result<(), YoukiCliError> {
        let log_path = self.log_path(container_id);

        // Ensure log directory exists
        if let Some(parent) = log_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let timestamp = Utc::now().to_rfc3339();
        let line = format!("{} {} {}\n", timestamp, stream, message);

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        Ok(())
    }

    // ==================== Resource Stats Methods ====================

    /// Get basic stats from cgroups.
    pub async fn get_stats(&self, container_id: &str) -> std::result::Result<ContainerStats, YoukiCliError> {
        let cgroup_path = PathBuf::from("/sys/fs/cgroup/youki").join(container_id);

        let cpu_usage = tokio::fs::read_to_string(cgroup_path.join("cpu.stat"))
            .await
            .ok()
            .map(|s| parse_cpu_usage(&s))
            .unwrap_or(0);

        let memory_usage = tokio::fs::read_to_string(cgroup_path.join("memory.current"))
            .await
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        Ok(ContainerStats {
            container_id: container_id.to_string(),
            cpu_usage_ns: cpu_usage,
            memory_usage_bytes: memory_usage,
        })
    }

    /// Clean up container bundle.
    async fn cleanup_bundle(&self, bundle_path: &Path) -> std::result::Result<(), YoukiCliError> {
        if bundle_path.exists() {
            tokio::fs::remove_dir_all(bundle_path).await?;
        }
        Ok(())
    }
}

/// Container resource statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStats {
    pub container_id: String,
    pub cpu_usage_ns: u64,
    pub memory_usage_bytes: u64,
}

fn parse_cpu_usage(content: &str) -> u64 {
    for line in content.lines() {
        if line.starts_with("usage_usec") {
            if let Some(val) = line.split_whitespace().nth(1) {
                return val.parse::<u64>().unwrap_or(0) * 1000;
            }
        }
    }
    0
}

#[async_trait]
impl ContainerRuntime for YoukiCliRuntime {
    async fn init_node(&self, node_id: NodeId) -> Result<()> {
        info!("YoukiCliRuntime: Initializing node {}", node_id);

        let bundle_dir = self.config.bundle_root.join(node_id.to_string());
        tokio::fs::create_dir_all(&bundle_dir)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to create bundle dir: {}", e)))?;

        let mut by_node = self.containers_by_node.write().await;
        by_node.entry(node_id).or_insert_with(Vec::new);

        Ok(())
    }

    async fn create_container(
        &self,
        config: &ContainerConfig,
        options: &CreateContainerOptions,
    ) -> Result<ContainerId> {
        let container_id = format!("{}-{}", config.name, Uuid::new_v4());

        info!(
            "YoukiCliRuntime: Creating container {} on node {}",
            container_id, options.node_id
        );

        let bundle_path = self.bundle_path(&options.node_id, &container_id);

        // Create bundle directory
        tokio::fs::create_dir_all(&bundle_path)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to create bundle: {}", e)))?;

        // Pull image and get rootfs
        info!("Pulling image: {}", config.image);
        let rootfs_source = self.image_manager.get_rootfs(&config.image)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to pull image: {}", e)))?;

        // Link rootfs to bundle
        let rootfs_dest = bundle_path.join("rootfs");
        if rootfs_dest.exists() {
            tokio::fs::remove_dir_all(&rootfs_dest).await.ok();
        }

        #[cfg(unix)]
        tokio::fs::symlink(&rootfs_source, &rootfs_dest)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to link rootfs: {}", e)))?;

        // Build OCI bundle (generates config.json)
        let mut builder = OciBundleBuilder::new(&bundle_path)
            .with_container_config(config)
            .skip_rootfs_setup();

        // Apply resource limits
        if config.resource_requests.cpu_cores > 0.0 {
            builder = builder.with_cpu_limit(config.resource_requests.cpu_cores);
        }
        if config.resource_requests.memory_mb > 0 {
            builder = builder.with_memory_limit(config.resource_requests.memory_mb);
        }

        builder.build()
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to build bundle: {}", e)))?;

        // Initialize log capture before starting container
        self.init_log_capture(&container_id)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("Failed to init log capture: {}", e)))?;

        // Write initial log entry
        self.write_log(&container_id, "system", &format!("Container {} starting", container_id))
            .await
            .ok(); // Don't fail on log write errors

        // youki create
        self.youki_create(&container_id, &bundle_path)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("youki create failed: {}", e)))?;

        // youki start
        self.youki_start(&container_id)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(format!("youki start failed: {}", e)))?;

        // Track container
        let state = ContainerState {
            id: container_id.clone(),
            node_id: options.node_id,
            bundle_path,
            status: "running".to_string(),
            pid: None,
        };

        self.containers.write().await.insert(container_id.clone(), state);
        self.containers_by_node.write().await
            .entry(options.node_id)
            .or_default()
            .push(container_id.clone());

        info!("Container {} created and started", container_id);
        Ok(container_id)
    }

    async fn stop_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("YoukiCliRuntime: Stopping container {}", container_id);

        // Write stop log entry
        self.write_log(container_id, "system", "Container stop requested")
            .await
            .ok();

        // Send SIGTERM
        if let Err(e) = self.youki_kill(container_id, "SIGTERM").await {
            warn!("SIGTERM failed: {}", e);
        }

        // Wait for stop or timeout
        let deadline = tokio::time::Instant::now() + self.config.stop_timeout;
        loop {
            match self.youki_state(container_id).await {
                Ok(state) if state.status == "stopped" => break,
                Ok(_) if tokio::time::Instant::now() >= deadline => {
                    warn!("Container {} didn't stop, sending SIGKILL", container_id);
                    self.write_log(container_id, "system", "Container timed out, sending SIGKILL")
                        .await
                        .ok();
                    self.youki_kill(container_id, "SIGKILL").await.ok();
                    break;
                }
                Ok(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(YoukiCliError::ContainerNotFound(_)) => break,
                Err(e) => return Err(OrchestrationError::RuntimeError(e.to_string())),
            }
        }

        // Stop any active log streams
        self.stop_log_stream(container_id).await;

        // Write final log entry
        self.write_log(container_id, "system", "Container stopped")
            .await
            .ok();

        // Update state
        if let Some(state) = self.containers.write().await.get_mut(container_id) {
            state.status = "stopped".to_string();
        }

        Ok(())
    }

    async fn remove_container(&self, container_id: &ContainerId) -> Result<()> {
        info!("YoukiCliRuntime: Removing container {}", container_id);

        // Stop any active log streams before removal
        self.stop_log_stream(container_id).await;

        // youki delete --force
        self.youki_delete(container_id, true)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))?;

        // Cleanup bundle
        if let Some(state) = self.containers.write().await.remove(container_id) {
            self.cleanup_bundle(&state.bundle_path).await.ok();

            let mut by_node = self.containers_by_node.write().await;
            if let Some(list) = by_node.get_mut(&state.node_id) {
                list.retain(|id| id != container_id);
            }
        }

        // Cleanup log directory
        let log_dir = self.log_dir(container_id);
        if log_dir.exists() {
            tokio::fs::remove_dir_all(&log_dir).await.ok();
        }

        Ok(())
    }

    async fn get_container_status(&self, container_id: &ContainerId) -> Result<ContainerStatus> {
        debug!("YoukiCliRuntime: Getting status for {}", container_id);

        match self.youki_state(container_id).await {
            Ok(state) => Ok(ContainerStatus {
                id: container_id.clone(),
                state: state.status,
                exit_code: None,
                error_message: None,
            }),
            Err(YoukiCliError::ContainerNotFound(_)) => {
                let containers = self.containers.read().await;
                if let Some(state) = containers.get(container_id) {
                    Ok(ContainerStatus {
                        id: container_id.clone(),
                        state: state.status.clone(),
                        exit_code: None,
                        error_message: None,
                    })
                } else {
                    Err(OrchestrationError::RuntimeError(format!(
                        "Container not found: {}", container_id
                    )))
                }
            }
            Err(e) => Ok(ContainerStatus {
                id: container_id.clone(),
                state: "unknown".to_string(),
                exit_code: None,
                error_message: Some(e.to_string()),
            }),
        }
    }

    async fn list_containers(&self, node_id: NodeId) -> Result<Vec<ContainerStatus>> {
        debug!("YoukiCliRuntime: Listing containers for node {}", node_id);

        let by_node = self.containers_by_node.read().await;
        let ids = by_node.get(&node_id).cloned().unwrap_or_default();
        drop(by_node);

        let mut statuses = Vec::new();
        for id in ids {
            match self.get_container_status(&id).await {
                Ok(status) => statuses.push(status),
                Err(e) => {
                    warn!("Failed to get status for {}: {}", id, e);
                    statuses.push(ContainerStatus {
                        id,
                        state: "unknown".to_string(),
                        exit_code: None,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(statuses)
    }

    async fn get_container_logs(
        &self,
        container_id: &ContainerId,
        options: &container_runtime_interface::LogOptions,
    ) -> Result<String> {
        // Convert interface LogOptions to our internal LogOptions
        let internal_options = LogOptions {
            tail: options.tail,
            timestamps: options.timestamps,
            since: options.since.clone(),
            until: options.until.clone(),
            follow: false, // follow mode not supported via this sync API
        };

        self.get_logs_with_options(container_id, &internal_options)
            .await
            .map_err(|e| OrchestrationError::RuntimeError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = YoukiCliConfig::default();
        assert_eq!(config.youki_binary, PathBuf::from("youki"));
        assert_eq!(config.command_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_cpu_usage() {
        let content = "usage_usec 12345\nuser_usec 10000\n";
        assert_eq!(parse_cpu_usage(content), 12345000);
    }

    #[test]
    fn test_youki_state_deserialize() {
        let json = r#"{
            "ociVersion": "1.0.2",
            "id": "test",
            "status": "running",
            "pid": 1234,
            "bundle": "/path/to/bundle"
        }"#;

        let state: YoukiState = serde_json::from_str(json).unwrap();
        assert_eq!(state.id, "test");
        assert_eq!(state.status, "running");
        assert_eq!(state.pid, Some(1234));
    }

    #[test]
    fn test_container_stats() {
        let stats = ContainerStats {
            container_id: "test".to_string(),
            cpu_usage_ns: 1000000,
            memory_usage_bytes: 1048576,
        };
        assert_eq!(stats.memory_usage_bytes, 1024 * 1024);
    }

    #[test]
    fn test_log_entry_serde() {
        let entry = LogEntry {
            timestamp: "2024-01-15T10:30:00.123Z".to_string(),
            stream: "stdout".to_string(),
            message: "Hello world".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.timestamp, entry.timestamp);
        assert_eq!(deserialized.stream, entry.stream);
        assert_eq!(deserialized.message, entry.message);
    }

    #[test]
    fn test_log_options_default() {
        let options = LogOptions::default();
        assert!(options.tail.is_none());
        assert!(!options.timestamps);
        assert!(options.since.is_none());
        assert!(options.until.is_none());
        assert!(!options.follow);
    }

    #[test]
    fn test_parse_log_line_full() {
        let line = "2024-01-15T10:30:00.123Z stdout Hello world";
        let entry = YoukiCliRuntime::parse_log_line(line).unwrap();
        assert_eq!(entry.timestamp, "2024-01-15T10:30:00.123Z");
        assert_eq!(entry.stream, "stdout");
        assert_eq!(entry.message, "Hello world");
    }

    #[test]
    fn test_parse_log_line_with_spaces() {
        let line = "2024-01-15T10:30:00Z stderr Error: something went wrong";
        let entry = YoukiCliRuntime::parse_log_line(line).unwrap();
        assert_eq!(entry.timestamp, "2024-01-15T10:30:00Z");
        assert_eq!(entry.stream, "stderr");
        assert_eq!(entry.message, "Error: something went wrong");
    }

    #[test]
    fn test_parse_log_line_no_stream() {
        // Timestamp with no stream defaults to stdout
        let line = "2024-01-15T10:30:00.123Z message-only";
        let entry = YoukiCliRuntime::parse_log_line(line).unwrap();
        assert_eq!(entry.timestamp, "2024-01-15T10:30:00.123Z");
        assert_eq!(entry.stream, "stdout"); // Default to stdout
        assert_eq!(entry.message, "message-only");
    }

    #[test]
    fn test_parse_log_line_empty() {
        assert!(YoukiCliRuntime::parse_log_line("").is_none());
    }

    #[test]
    fn test_parse_log_line_bare_message() {
        // Lines without timestamp get current time
        let entry = YoukiCliRuntime::parse_log_line("bare message").unwrap();
        assert_eq!(entry.stream, "stdout");
        assert_eq!(entry.message, "bare message");
        // Timestamp should be valid RFC3339
        assert!(entry.timestamp.contains('T'));
    }
}
