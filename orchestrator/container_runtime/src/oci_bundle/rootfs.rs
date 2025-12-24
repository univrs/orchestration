//! Rootfs preparation utilities for OCI bundles.
//!
//! This module provides utilities for creating and managing the root filesystem
//! of OCI containers, including:
//! - Creating directory structure
//! - Setting up device nodes
//! - Creating symlinks
//! - Preparing a minimal rootfs for testing

use std::path::{Path, PathBuf};
use std::io;
use thiserror::Error;
use tracing::{debug, info};

/// Errors that can occur during rootfs operations.
#[derive(Debug, Error)]
pub enum RootfsError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: io::Error,
    },

    #[error("Failed to create symlink {link} -> {target}: {source}")]
    CreateSymlink {
        link: PathBuf,
        target: PathBuf,
        source: io::Error,
    },

    #[error("Rootfs path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Failed to extract image: {0}")]
    ImageExtraction(String),
}

/// Standard directories in a Linux rootfs.
pub const STANDARD_DIRS: &[&str] = &[
    "bin",
    "dev",
    "dev/pts",
    "dev/shm",
    "etc",
    "home",
    "lib",
    "lib64",
    "mnt",
    "opt",
    "proc",
    "root",
    "run",
    "sbin",
    "srv",
    "sys",
    "tmp",
    "usr",
    "usr/bin",
    "usr/lib",
    "usr/lib64",
    "usr/sbin",
    "usr/share",
    "var",
    "var/cache",
    "var/lib",
    "var/log",
    "var/run",
    "var/tmp",
];

/// Device node specifications for /dev.
#[derive(Debug, Clone)]
pub struct DeviceNode {
    pub path: &'static str,
    pub device_type: char, // 'c' for char, 'b' for block
    pub major: u32,
    pub minor: u32,
    pub mode: u32,
}

/// Standard device nodes for a container.
pub const STANDARD_DEVICES: &[DeviceNode] = &[
    DeviceNode { path: "dev/null", device_type: 'c', major: 1, minor: 3, mode: 0o666 },
    DeviceNode { path: "dev/zero", device_type: 'c', major: 1, minor: 5, mode: 0o666 },
    DeviceNode { path: "dev/full", device_type: 'c', major: 1, minor: 7, mode: 0o666 },
    DeviceNode { path: "dev/random", device_type: 'c', major: 1, minor: 8, mode: 0o666 },
    DeviceNode { path: "dev/urandom", device_type: 'c', major: 1, minor: 9, mode: 0o666 },
    DeviceNode { path: "dev/tty", device_type: 'c', major: 5, minor: 0, mode: 0o666 },
    DeviceNode { path: "dev/console", device_type: 'c', major: 5, minor: 1, mode: 0o620 },
    DeviceNode { path: "dev/ptmx", device_type: 'c', major: 5, minor: 2, mode: 0o666 },
];

/// Standard symlinks in /dev.
pub const DEV_SYMLINKS: &[(&str, &str)] = &[
    ("dev/fd", "/proc/self/fd"),
    ("dev/stdin", "/proc/self/fd/0"),
    ("dev/stdout", "/proc/self/fd/1"),
    ("dev/stderr", "/proc/self/fd/2"),
];

/// Builder for creating rootfs directories.
pub struct RootfsBuilder {
    path: PathBuf,
    create_dirs: bool,
    create_dev_symlinks: bool,
    create_etc_files: bool,
}

impl RootfsBuilder {
    /// Create a new rootfs builder for the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            create_dirs: true,
            create_dev_symlinks: true,
            create_etc_files: true,
        }
    }

    /// Skip creating standard directories.
    pub fn skip_dirs(mut self) -> Self {
        self.create_dirs = false;
        self
    }

    /// Skip creating /dev symlinks.
    pub fn skip_dev_symlinks(mut self) -> Self {
        self.create_dev_symlinks = false;
        self
    }

    /// Skip creating basic /etc files.
    pub fn skip_etc_files(mut self) -> Self {
        self.create_etc_files = false;
        self
    }

    /// Build the rootfs structure.
    pub fn build(self) -> Result<Rootfs, RootfsError> {
        info!("Creating rootfs at {:?}", self.path);

        // Create the rootfs directory itself
        std::fs::create_dir_all(&self.path).map_err(|e| RootfsError::CreateDir {
            path: self.path.clone(),
            source: e,
        })?;

        // Create standard directories
        if self.create_dirs {
            self.create_standard_dirs()?;
        }

        // Create /dev symlinks
        if self.create_dev_symlinks {
            self.create_dev_symlinks_impl()?;
        }

        // Create basic /etc files
        if self.create_etc_files {
            self.create_etc_files_impl()?;
        }

        Ok(Rootfs { path: self.path })
    }

    fn create_standard_dirs(&self) -> Result<(), RootfsError> {
        for dir in STANDARD_DIRS {
            let full_path = self.path.join(dir);
            debug!("Creating directory: {:?}", full_path);
            std::fs::create_dir_all(&full_path).map_err(|e| RootfsError::CreateDir {
                path: full_path,
                source: e,
            })?;
        }
        Ok(())
    }

    fn create_dev_symlinks_impl(&self) -> Result<(), RootfsError> {
        for (link, target) in DEV_SYMLINKS {
            let link_path = self.path.join(link);
            let target_path = PathBuf::from(target);

            // Remove existing symlink if present
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                let _ = std::fs::remove_file(&link_path);
            }

            debug!("Creating symlink: {:?} -> {:?}", link_path, target_path);

            #[cfg(unix)]
            std::os::unix::fs::symlink(&target_path, &link_path).map_err(|e| {
                RootfsError::CreateSymlink {
                    link: link_path,
                    target: target_path,
                    source: e,
                }
            })?;

            #[cfg(not(unix))]
            {
                // On non-Unix systems, create a placeholder file
                std::fs::write(&link_path, target).map_err(|e| RootfsError::CreateSymlink {
                    link: link_path,
                    target: target_path,
                    source: e,
                })?;
            }
        }
        Ok(())
    }

    fn create_etc_files_impl(&self) -> Result<(), RootfsError> {
        let etc_path = self.path.join("etc");
        std::fs::create_dir_all(&etc_path).map_err(|e| RootfsError::CreateDir {
            path: etc_path.clone(),
            source: e,
        })?;

        // Create /etc/passwd
        let passwd_content = "root:x:0:0:root:/root:/bin/sh\nnobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n";
        std::fs::write(etc_path.join("passwd"), passwd_content)?;

        // Create /etc/group
        let group_content = "root:x:0:\nnogroup:x:65534:\n";
        std::fs::write(etc_path.join("group"), group_content)?;

        // Create /etc/shadow (empty but required)
        let shadow_content = "root:!:19000:0:99999:7:::\n";
        std::fs::write(etc_path.join("shadow"), shadow_content)?;

        // Create /etc/hostname
        std::fs::write(etc_path.join("hostname"), "container\n")?;

        // Create /etc/hosts
        let hosts_content = "127.0.0.1\tlocalhost\n::1\tlocalhost\n";
        std::fs::write(etc_path.join("hosts"), hosts_content)?;

        // Create /etc/resolv.conf
        let resolv_content = "nameserver 8.8.8.8\nnameserver 8.8.4.4\n";
        std::fs::write(etc_path.join("resolv.conf"), resolv_content)?;

        Ok(())
    }
}

/// Represents a prepared rootfs directory.
pub struct Rootfs {
    pub(crate) path: PathBuf,
}

impl Rootfs {
    /// Create a Rootfs wrapper for an existing path.
    /// Use RootfsBuilder for creating a new rootfs with proper structure.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the path to the rootfs.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if the rootfs exists and has the basic structure.
    pub fn validate(&self) -> Result<(), RootfsError> {
        if !self.path.exists() {
            return Err(RootfsError::PathNotFound(self.path.clone()));
        }

        // Check for essential directories
        let essential = ["bin", "etc", "proc", "sys", "dev"];
        for dir in essential {
            let dir_path = self.path.join(dir);
            if !dir_path.exists() {
                return Err(RootfsError::PathNotFound(dir_path));
            }
        }

        Ok(())
    }

    /// Copy a file into the rootfs.
    pub fn copy_file(&self, src: &Path, dest: &str) -> Result<(), RootfsError> {
        let dest_path = self.path.join(dest.trim_start_matches('/'));

        // Create parent directory if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RootfsError::CreateDir {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::copy(src, &dest_path)?;
        Ok(())
    }

    /// Write content to a file in the rootfs.
    pub fn write_file(&self, dest: &str, content: &str) -> Result<(), RootfsError> {
        let dest_path = self.path.join(dest.trim_start_matches('/'));

        // Create parent directory if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RootfsError::CreateDir {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::write(&dest_path, content)?;
        Ok(())
    }

    /// Create a directory in the rootfs.
    pub fn create_dir(&self, path: &str) -> Result<(), RootfsError> {
        let full_path = self.path.join(path.trim_start_matches('/'));
        std::fs::create_dir_all(&full_path).map_err(|e| RootfsError::CreateDir {
            path: full_path,
            source: e,
        })?;
        Ok(())
    }

    /// Remove the rootfs directory.
    pub fn cleanup(self) -> Result<(), RootfsError> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)?;
        }
        Ok(())
    }
}

/// Extract an OCI image layer to the rootfs.
///
/// This is a placeholder for actual image extraction.
/// In a real implementation, this would:
/// 1. Pull the image from a registry
/// 2. Extract each layer in order
/// 3. Apply whiteouts for deleted files
pub async fn extract_image_layer(
    _rootfs: &Path,
    _layer_path: &Path,
) -> Result<(), RootfsError> {
    // Placeholder - actual implementation would use tar extraction
    // with proper handling of OCI whiteout files
    Err(RootfsError::ImageExtraction(
        "Image extraction not yet implemented".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_rootfs_builder() {
        let temp = TempDir::new().unwrap();
        let rootfs_path = temp.path().join("rootfs");

        let rootfs = RootfsBuilder::new(&rootfs_path)
            .build()
            .expect("Failed to build rootfs");

        // Verify basic structure
        assert!(rootfs_path.join("bin").exists());
        assert!(rootfs_path.join("etc").exists());
        assert!(rootfs_path.join("proc").exists());
        assert!(rootfs_path.join("dev").exists());
        assert!(rootfs_path.join("tmp").exists());

        // Verify etc files
        assert!(rootfs_path.join("etc/passwd").exists());
        assert!(rootfs_path.join("etc/hosts").exists());

        rootfs.validate().expect("Validation failed");
    }

    #[test]
    fn test_rootfs_write_file() {
        let temp = TempDir::new().unwrap();
        let rootfs_path = temp.path().join("rootfs");

        let rootfs = RootfsBuilder::new(&rootfs_path)
            .build()
            .expect("Failed to build rootfs");

        rootfs
            .write_file("/custom/test.txt", "hello world")
            .expect("Failed to write file");

        let content = fs::read_to_string(rootfs_path.join("custom/test.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_rootfs_cleanup() {
        let temp = TempDir::new().unwrap();
        let rootfs_path = temp.path().join("rootfs");

        let rootfs = RootfsBuilder::new(&rootfs_path)
            .build()
            .expect("Failed to build rootfs");

        assert!(rootfs_path.exists());

        rootfs.cleanup().expect("Failed to cleanup");

        assert!(!rootfs_path.exists());
    }

    #[test]
    fn test_skip_options() {
        let temp = TempDir::new().unwrap();
        let rootfs_path = temp.path().join("rootfs");

        let _rootfs = RootfsBuilder::new(&rootfs_path)
            .skip_etc_files()
            .build()
            .expect("Failed to build rootfs");

        // etc dir should exist but not the files
        assert!(rootfs_path.join("etc").exists());
        // passwd may or may not exist depending on skip behavior
    }
}
