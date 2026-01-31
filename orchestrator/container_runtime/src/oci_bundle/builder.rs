//! OCI Bundle Builder
//!
//! This module provides a high-level builder for creating OCI bundles
//! from container configurations.

use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};

use orchestrator_shared_types::ContainerConfig;

use super::rootfs::{Rootfs, RootfsBuilder, RootfsError};
use super::spec::{
    Capabilities, CpuResources, Device, Linux, MemoryResources, Mount, Namespace, OciSpec,
    PidsResources, Process, Resources, Root, User,
};

/// Errors that can occur during bundle operations.
#[derive(Debug, Error)]
pub enum BundleError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Rootfs error: {0}")]
    Rootfs(#[from] RootfsError),

    #[error("Bundle path already exists: {0}")]
    PathExists(PathBuf),

    #[error("Bundle path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Result type for bundle operations.
pub type BundleResult<T> = Result<T, BundleError>;

/// A created OCI bundle.
pub struct OciBundle {
    /// Path to the bundle directory
    path: PathBuf,
    /// The rootfs within the bundle
    rootfs: Rootfs,
    /// The OCI spec for the bundle
    spec: OciSpec,
}

impl OciBundle {
    /// Get the bundle path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the rootfs path.
    pub fn rootfs_path(&self) -> &Path {
        self.rootfs.path()
    }

    /// Get the config.json path.
    pub fn config_path(&self) -> PathBuf {
        self.path.join("config.json")
    }

    /// Get the OCI spec.
    pub fn spec(&self) -> &OciSpec {
        &self.spec
    }

    /// Update the OCI spec and rewrite config.json.
    pub fn update_spec(&mut self, spec: OciSpec) -> BundleResult<()> {
        self.spec = spec;
        self.write_config()
    }

    /// Write the config.json file.
    fn write_config(&self) -> BundleResult<()> {
        let config_path = self.config_path();
        let json = serde_json::to_string_pretty(&self.spec)?;
        std::fs::write(&config_path, json)?;
        Ok(())
    }

    /// Clean up the bundle directory.
    pub fn cleanup(self) -> BundleResult<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)?;
        }
        Ok(())
    }
}

/// Builder for creating OCI bundles.
pub struct OciBundleBuilder {
    path: PathBuf,
    container_config: Option<ContainerConfig>,
    hostname: Option<String>,
    cpu_cores: Option<f32>,
    memory_mb: Option<u64>,
    pids_limit: Option<i64>,
    privileged: bool,
    additional_mounts: Vec<Mount>,
    additional_env: Vec<String>,
    skip_rootfs_setup: bool,
    annotations: std::collections::HashMap<String, String>,
}

impl OciBundleBuilder {
    /// Create a new bundle builder for the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            container_config: None,
            hostname: None,
            cpu_cores: None,
            memory_mb: None,
            pids_limit: Some(1024), // Default PID limit
            privileged: false,
            additional_mounts: Vec::new(),
            additional_env: Vec::new(),
            skip_rootfs_setup: false,
            annotations: std::collections::HashMap::new(),
        }
    }

    /// Set the container configuration.
    pub fn with_container_config(mut self, config: &ContainerConfig) -> Self {
        self.container_config = Some(config.clone());
        self
    }

    /// Set the container hostname.
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self
    }

    /// Set CPU limit in cores (e.g., 0.5 = half a core).
    pub fn with_cpu_limit(mut self, cores: f32) -> Self {
        self.cpu_cores = Some(cores);
        self
    }

    /// Set memory limit in MB.
    pub fn with_memory_limit(mut self, mb: u64) -> Self {
        self.memory_mb = Some(mb);
        self
    }

    /// Set PID limit.
    pub fn with_pids_limit(mut self, limit: i64) -> Self {
        self.pids_limit = Some(limit);
        self
    }

    /// Run as a privileged container (all capabilities).
    pub fn privileged(mut self) -> Self {
        self.privileged = true;
        self
    }

    /// Add additional mount points.
    pub fn with_mount(mut self, mount: Mount) -> Self {
        self.additional_mounts.push(mount);
        self
    }

    /// Add additional environment variables.
    pub fn with_env(mut self, env: impl Into<String>) -> Self {
        self.additional_env.push(env.into());
        self
    }

    /// Skip rootfs setup (for pre-populated rootfs).
    pub fn skip_rootfs_setup(mut self) -> Self {
        self.skip_rootfs_setup = true;
        self
    }

    /// Add an annotation.
    pub fn with_annotation(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.annotations.insert(key.into(), value.into());
        self
    }

    /// Build the OCI bundle.
    pub fn build(self) -> BundleResult<OciBundle> {
        info!("Building OCI bundle at {:?}", self.path);

        // Create bundle directory
        if self.path.exists() {
            return Err(BundleError::PathExists(self.path.clone()));
        }
        std::fs::create_dir_all(&self.path)?;

        // Create rootfs
        let rootfs_path = self.path.join("rootfs");
        let rootfs = if self.skip_rootfs_setup {
            std::fs::create_dir_all(&rootfs_path)?;
            Rootfs::from_path(rootfs_path)
        } else {
            RootfsBuilder::new(&rootfs_path).build()?
        };

        // Build the OCI spec
        let spec = self.build_spec()?;

        // Write config.json
        let config_path = self.path.join("config.json");
        let json = serde_json::to_string_pretty(&spec)?;
        std::fs::write(&config_path, &json)?;

        debug!("Bundle created successfully");

        Ok(OciBundle {
            path: self.path,
            rootfs,
            spec,
        })
    }

    /// Build the OCI specification.
    fn build_spec(&self) -> BundleResult<OciSpec> {
        let config = self.container_config.as_ref();

        // Build process
        let process = self.build_process(config)?;

        // Build root
        let root = Root {
            path: "rootfs".to_string(),
            readonly: false,
        };

        // Build mounts
        let mounts = self.build_mounts();

        // Build Linux config
        let linux = self.build_linux(config)?;

        // Determine hostname
        let hostname = self
            .hostname
            .clone()
            .or_else(|| config.map(|c| c.name.clone()));

        let spec = OciSpec {
            oci_version: "1.0.2".to_string(),
            root: Some(root),
            process: Some(process),
            hostname,
            mounts,
            linux: Some(linux),
            annotations: self.annotations.clone(),
        };

        Ok(spec)
    }

    /// Build the process configuration.
    fn build_process(&self, config: Option<&ContainerConfig>) -> BundleResult<Process> {
        // Build args
        let args = if let Some(c) = config {
            let mut args = c
                .command
                .clone()
                .unwrap_or_else(|| vec!["/bin/sh".to_string()]);
            if let Some(extra) = &c.args {
                args.extend(extra.clone());
            }
            args
        } else {
            vec!["/bin/sh".to_string()]
        };

        // Build environment
        let mut env = vec![
            "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
            "TERM=xterm".to_string(),
        ];

        if let Some(c) = config {
            for (k, v) in &c.env_vars {
                env.push(format!("{}={}", k, v));
            }
        }

        env.extend(self.additional_env.clone());

        // Build capabilities
        let capabilities = if self.privileged {
            Some(Capabilities::privileged())
        } else {
            Some(Capabilities::default())
        };

        Ok(Process {
            terminal: false,
            user: User::default(),
            args,
            env,
            cwd: "/".to_string(),
            capabilities,
            rlimits: Vec::new(),
            no_new_privileges: !self.privileged,
        })
    }

    /// Build mount configurations.
    fn build_mounts(&self) -> Vec<Mount> {
        let mut mounts = vec![
            Mount::proc(),
            Mount::tmpfs("/dev"),
            Mount::devpts(),
            Mount::shm(),
            Mount::mqueue(),
            Mount::sysfs(),
            Mount::cgroup(),
        ];

        mounts.extend(self.additional_mounts.clone());

        mounts
    }

    /// Build Linux-specific configuration.
    fn build_linux(&self, config: Option<&ContainerConfig>) -> BundleResult<Linux> {
        // Namespaces
        let namespaces = vec![
            Namespace::pid(),
            Namespace::network(),
            Namespace::ipc(),
            Namespace::uts(),
            Namespace::mount(),
        ];

        // Devices
        let devices = vec![
            Device::null(),
            Device::zero(),
            Device::full(),
            Device::random(),
            Device::urandom(),
            Device::tty(),
        ];

        // Resources
        let resources = self.build_resources(config);

        // Masked and readonly paths for security
        let masked_paths = vec![
            "/proc/acpi".to_string(),
            "/proc/kcore".to_string(),
            "/proc/keys".to_string(),
            "/proc/latency_stats".to_string(),
            "/proc/timer_list".to_string(),
            "/proc/timer_stats".to_string(),
            "/proc/sched_debug".to_string(),
            "/proc/scsi".to_string(),
            "/sys/firmware".to_string(),
        ];

        let readonly_paths = vec![
            "/proc/bus".to_string(),
            "/proc/fs".to_string(),
            "/proc/irq".to_string(),
            "/proc/sys".to_string(),
            "/proc/sysrq-trigger".to_string(),
        ];

        Ok(Linux {
            namespaces,
            uid_mappings: Vec::new(),
            gid_mappings: Vec::new(),
            devices,
            cgroups_path: None,
            resources: Some(resources),
            masked_paths: if self.privileged {
                Vec::new()
            } else {
                masked_paths
            },
            readonly_paths: if self.privileged {
                Vec::new()
            } else {
                readonly_paths
            },
            seccomp: None, // Could add default seccomp profile
        })
    }

    /// Build resource limits.
    fn build_resources(&self, config: Option<&ContainerConfig>) -> Resources {
        // Get CPU from config or builder override
        let cpu_cores = self
            .cpu_cores
            .or_else(|| config.map(|c| c.resource_requests.cpu_cores))
            .filter(|&c| c > 0.0);

        // Get memory from config or builder override
        let memory_mb = self
            .memory_mb
            .or_else(|| config.map(|c| c.resource_requests.memory_mb))
            .filter(|&m| m > 0);

        Resources {
            cpu: cpu_cores.map(CpuResources::from_cores),
            memory: memory_mb.map(MemoryResources::from_mb),
            block_io: None,
            pids: self.pids_limit.map(|limit| PidsResources { limit }),
        }
    }
}

/// Create a bundle from a ContainerConfig with default settings.
pub fn create_bundle_from_config(
    path: impl Into<PathBuf>,
    config: &ContainerConfig,
) -> BundleResult<OciBundle> {
    OciBundleBuilder::new(path)
        .with_container_config(config)
        .build()
}

/// Async version of bundle creation.
pub async fn create_bundle_async(
    path: PathBuf,
    config: ContainerConfig,
) -> BundleResult<OciBundle> {
    tokio::task::spawn_blocking(move || {
        OciBundleBuilder::new(path)
            .with_container_config(&config)
            .build()
    })
    .await
    .map_err(|e| BundleError::InvalidConfig(format!("Task join error: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_shared_types::NodeResources;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn test_container_config() -> ContainerConfig {
        ContainerConfig {
            name: "test-container".to_string(),
            image: "alpine:latest".to_string(),
            command: Some(vec!["/bin/sh".to_string()]),
            args: Some(vec!["-c".to_string(), "echo hello".to_string()]),
            env_vars: HashMap::from([("FOO".to_string(), "bar".to_string())]),
            ports: Vec::new(),
            resource_requests: NodeResources {
                cpu_cores: 0.5,
                memory_mb: 256,
                disk_mb: 0,
            },
        }
    }

    #[test]
    fn test_bundle_builder() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let config = test_container_config();
        let bundle = OciBundleBuilder::new(&bundle_path)
            .with_container_config(&config)
            .with_hostname("my-container")
            .build()
            .expect("Failed to build bundle");

        // Verify bundle structure
        assert!(bundle_path.join("config.json").exists());
        assert!(bundle_path.join("rootfs").exists());

        // Verify config.json content
        let spec = bundle.spec();
        assert_eq!(spec.oci_version, "1.0.2");
        assert_eq!(spec.hostname, Some("my-container".to_string()));

        // Verify process
        let process = spec.process.as_ref().unwrap();
        assert_eq!(process.args, vec!["/bin/sh", "-c", "echo hello"]);
        assert!(process.env.iter().any(|e| e == "FOO=bar"));

        // Verify resources
        let linux = spec.linux.as_ref().unwrap();
        let resources = linux.resources.as_ref().unwrap();
        assert!(resources.cpu.is_some());
        assert!(resources.memory.is_some());
    }

    #[test]
    fn test_bundle_with_resource_limits() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let bundle = OciBundleBuilder::new(&bundle_path)
            .with_cpu_limit(2.0)
            .with_memory_limit(512)
            .with_pids_limit(100)
            .build()
            .expect("Failed to build bundle");

        let linux = bundle.spec().linux.as_ref().unwrap();
        let resources = linux.resources.as_ref().unwrap();

        let cpu = resources.cpu.as_ref().unwrap();
        assert_eq!(cpu.quota, Some(200_000)); // 2.0 cores * 100_000

        let memory = resources.memory.as_ref().unwrap();
        assert_eq!(memory.limit, Some(512 * 1024 * 1024));

        let pids = resources.pids.as_ref().unwrap();
        assert_eq!(pids.limit, 100);
    }

    #[test]
    fn test_privileged_bundle() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let bundle = OciBundleBuilder::new(&bundle_path)
            .privileged()
            .build()
            .expect("Failed to build bundle");

        let process = bundle.spec().process.as_ref().unwrap();
        let caps = process.capabilities.as_ref().unwrap();

        // Should have all capabilities
        assert!(caps.bounding.len() > 10);
        assert!(caps.bounding.contains(&"CAP_SYS_ADMIN".to_string()));

        // Should not have no_new_privileges
        assert!(!process.no_new_privileges);

        // Should not have masked paths
        let linux = bundle.spec().linux.as_ref().unwrap();
        assert!(linux.masked_paths.is_empty());
    }

    #[test]
    fn test_bundle_cleanup() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let bundle = OciBundleBuilder::new(&bundle_path)
            .build()
            .expect("Failed to build bundle");

        assert!(bundle_path.exists());

        bundle.cleanup().expect("Failed to cleanup");

        assert!(!bundle_path.exists());
    }

    #[test]
    fn test_bundle_path_exists_error() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        // Create the first bundle
        let _bundle1 = OciBundleBuilder::new(&bundle_path)
            .build()
            .expect("Failed to build first bundle");

        // Try to create another bundle at the same path
        let result = OciBundleBuilder::new(&bundle_path).build();

        assert!(matches!(result, Err(BundleError::PathExists(_))));
    }

    #[test]
    fn test_create_bundle_from_config() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let config = test_container_config();
        let bundle =
            create_bundle_from_config(&bundle_path, &config).expect("Failed to create bundle");

        assert!(bundle.path().exists());
        assert_eq!(bundle.spec().hostname, Some("test-container".to_string()));
    }
}
