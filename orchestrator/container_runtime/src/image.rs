//! Image Manager for pulling and extracting container images.
//!
//! This module provides functionality for:
//! - Parsing image references (registry/repo:tag format)
//! - Pulling manifests from Docker Hub
//! - Pulling and extracting image layers
//! - Managing a local image cache

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::info;

#[cfg(feature = "image-pull")]
use std::io::Write;

#[cfg(feature = "image-pull")]
use tracing::{debug, warn};

#[cfg(feature = "image-pull")]
use {
    flate2::read::GzDecoder,
    futures_util::StreamExt,
    reqwest::Client,
    sha2::{Digest, Sha256},
    tar::Archive,
};

/// Errors that can occur during image operations.
#[derive(Debug, Error)]
pub enum ImageError {
    #[error("Invalid image reference: {0}")]
    InvalidReference(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("HTTP error: {0}")]
    #[cfg(feature = "image-pull")]
    Http(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Registry error: {status} - {message}")]
    Registry { status: u16, message: String },

    #[error("Layer digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    #[error("Manifest not found for image: {0}")]
    ManifestNotFound(String),

    #[error("Layer not found: {0}")]
    LayerNotFound(String),

    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[error("Image pull feature not enabled")]
    FeatureNotEnabled,
}

/// Parsed image reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReference {
    /// Registry host (e.g., "registry-1.docker.io")
    pub registry: String,
    /// Repository name (e.g., "library/alpine")
    pub repository: String,
    /// Tag or digest (e.g., "latest" or "sha256:...")
    pub tag: String,
}

impl ImageReference {
    /// Create a new image reference.
    pub fn new(
        registry: impl Into<String>,
        repository: impl Into<String>,
        tag: impl Into<String>,
    ) -> Self {
        Self {
            registry: registry.into(),
            repository: repository.into(),
            tag: tag.into(),
        }
    }

    /// Get the full image name (without registry).
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
    }
}

impl std::fmt::Display for ImageReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}:{}", self.registry, self.repository, self.tag)
    }
}

/// Docker Registry manifest (v2 schema 2).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: i32,
    pub media_type: Option<String>,
    pub config: ManifestConfig,
    pub layers: Vec<ManifestLayer>,
}

/// Configuration blob reference in manifest.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestConfig {
    pub media_type: String,
    pub size: i64,
    pub digest: String,
}

/// Layer reference in manifest.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLayer {
    pub media_type: String,
    pub size: i64,
    pub digest: String,
}

/// Docker Hub authentication token response.
#[cfg(feature = "image-pull")]
#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
    #[allow(dead_code)]
    expires_in: Option<i64>,
}

/// Image manager for pulling and caching container images.
pub struct ImageManager {
    /// Directory for caching downloaded images
    cache_dir: PathBuf,
    /// HTTP client for registry requests
    #[cfg(feature = "image-pull")]
    client: Client,
}

impl ImageManager {
    /// Create a new image manager with the given cache directory.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Result<Self, ImageError> {
        let cache_dir = cache_dir.into();
        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            cache_dir,
            #[cfg(feature = "image-pull")]
            client: Client::builder()
                .user_agent("ai-native-orchestrator/0.1")
                .build()?,
        })
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Parse an image reference string into components.
    ///
    /// Supports formats:
    /// - `alpine` -> registry-1.docker.io/library/alpine:latest
    /// - `alpine:3.19` -> registry-1.docker.io/library/alpine:3.19
    /// - `nginx:latest` -> registry-1.docker.io/library/nginx:latest
    /// - `myuser/myimage:v1` -> registry-1.docker.io/myuser/myimage:v1
    /// - `ghcr.io/owner/repo:tag` -> ghcr.io/owner/repo:tag
    pub fn parse_image_ref(image: &str) -> Result<ImageReference, ImageError> {
        let image = image.trim();
        if image.is_empty() {
            return Err(ImageError::InvalidReference(
                "empty image reference".to_string(),
            ));
        }

        // Check for digest reference
        let (name_part, tag) = if image.contains('@') {
            let parts: Vec<&str> = image.splitn(2, '@').collect();
            (parts[0], parts.get(1).unwrap_or(&"latest").to_string())
        } else if image.contains(':') {
            // Check if colon is part of registry (e.g., localhost:5000/image)
            let parts: Vec<&str> = image.splitn(2, '/').collect();
            if parts.len() > 1 && parts[0].contains(':') && !parts[0].contains('.') {
                // Likely a port number, not a tag
                (image, "latest".to_string())
            } else {
                let colon_parts: Vec<&str> = image.rsplitn(2, ':').collect();
                if colon_parts.len() == 2 {
                    (colon_parts[1], colon_parts[0].to_string())
                } else {
                    (image, "latest".to_string())
                }
            }
        } else {
            (image, "latest".to_string())
        };

        // Parse registry and repository
        let parts: Vec<&str> = name_part.split('/').collect();

        let (registry, repository) = match parts.len() {
            1 => {
                // Simple name like "alpine"
                (
                    "registry-1.docker.io".to_string(),
                    format!("library/{}", parts[0]),
                )
            }
            2 => {
                // Could be user/repo or registry/repo
                let first = parts[0];
                if first.contains('.') || first.contains(':') || first == "localhost" {
                    // It's a registry
                    (first.to_string(), parts[1].to_string())
                } else {
                    // It's user/repo on Docker Hub
                    (
                        "registry-1.docker.io".to_string(),
                        format!("{}/{}", parts[0], parts[1]),
                    )
                }
            }
            _ => {
                // registry/path/to/repo
                let registry = parts[0].to_string();
                let repository = parts[1..].join("/");
                (registry, repository)
            }
        };

        Ok(ImageReference {
            registry,
            repository,
            tag,
        })
    }

    /// Get an authentication token for Docker Hub.
    #[cfg(feature = "image-pull")]
    async fn get_docker_hub_token(&self, repository: &str) -> Result<String, ImageError> {
        let auth_url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull",
            repository
        );

        debug!("Fetching Docker Hub token for {}", repository);

        let response = self.client.get(&auth_url).send().await?;

        if !response.status().is_success() {
            return Err(ImageError::Registry {
                status: response.status().as_u16(),
                message: "Failed to get authentication token".to_string(),
            });
        }

        let token_response: TokenResponse = response.json().await?;
        Ok(token_response.token)
    }

    /// Pull the manifest for an image.
    #[cfg(feature = "image-pull")]
    pub async fn pull_manifest(&self, image_ref: &ImageReference) -> Result<Manifest, ImageError> {
        info!("Pulling manifest for {}", image_ref);

        // Get authentication token (Docker Hub only for now)
        let token = if image_ref.registry == "registry-1.docker.io" {
            Some(self.get_docker_hub_token(&image_ref.repository).await?)
        } else {
            None
        };

        let manifest_url = format!(
            "https://{}/v2/{}/manifests/{}",
            image_ref.registry, image_ref.repository, image_ref.tag
        );

        debug!("Fetching manifest from {}", manifest_url);

        let mut request = self.client.get(&manifest_url).header(
            "Accept",
            "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json",
        );

        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ImageError::Registry { status, message });
        }

        let manifest: Manifest = response.json().await?;
        debug!("Got manifest with {} layers", manifest.layers.len());

        Ok(manifest)
    }

    /// Pull manifest (stub for when feature is disabled).
    #[cfg(not(feature = "image-pull"))]
    pub async fn pull_manifest(&self, _image_ref: &ImageReference) -> Result<Manifest, ImageError> {
        Err(ImageError::FeatureNotEnabled)
    }

    /// Pull a single layer blob.
    #[cfg(feature = "image-pull")]
    pub async fn pull_layer(
        &self,
        image_ref: &ImageReference,
        layer: &ManifestLayer,
    ) -> Result<PathBuf, ImageError> {
        let digest = &layer.digest;
        let layer_path = self.cache_dir.join("layers").join(digest.replace(':', "_"));

        // Check if layer is already cached
        if layer_path.exists() {
            debug!("Layer {} already cached", digest);
            return Ok(layer_path);
        }

        info!("Pulling layer {} ({} bytes)", digest, layer.size);

        // Ensure layers directory exists
        std::fs::create_dir_all(self.cache_dir.join("layers"))?;

        // Get authentication token
        let token = if image_ref.registry == "registry-1.docker.io" {
            Some(self.get_docker_hub_token(&image_ref.repository).await?)
        } else {
            None
        };

        let blob_url = format!(
            "https://{}/v2/{}/blobs/{}",
            image_ref.registry, image_ref.repository, digest
        );

        debug!("Fetching blob from {}", blob_url);

        let mut request = self.client.get(&blob_url);
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ImageError::Registry { status, message });
        }

        // Stream the response to a file while computing digest
        let mut hasher = Sha256::new();
        let mut file = std::fs::File::create(&layer_path)?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            file.write_all(&chunk)?;
        }

        // Verify digest
        let computed_digest = format!("sha256:{}", hex::encode(hasher.finalize()));
        if computed_digest != *digest {
            std::fs::remove_file(&layer_path)?;
            return Err(ImageError::DigestMismatch {
                expected: digest.clone(),
                actual: computed_digest,
            });
        }

        info!("Layer {} downloaded and verified", digest);
        Ok(layer_path)
    }

    /// Pull layer (stub for when feature is disabled).
    #[cfg(not(feature = "image-pull"))]
    pub async fn pull_layer(
        &self,
        _image_ref: &ImageReference,
        _layer: &ManifestLayer,
    ) -> Result<PathBuf, ImageError> {
        Err(ImageError::FeatureNotEnabled)
    }

    /// Extract all layers to create a rootfs.
    #[cfg(feature = "image-pull")]
    pub async fn extract_layers(
        &self,
        image_ref: &ImageReference,
        manifest: &Manifest,
    ) -> Result<PathBuf, ImageError> {
        let image_id = format!(
            "{}_{}",
            image_ref.repository.replace('/', "_"),
            image_ref.tag
        );
        let rootfs_path = self.cache_dir.join("rootfs").join(&image_id);

        // Check if already extracted
        if rootfs_path.exists() {
            debug!("Rootfs already exists for {}", image_ref);
            return Ok(rootfs_path);
        }

        info!(
            "Extracting {} layers to {:?}",
            manifest.layers.len(),
            rootfs_path
        );

        // Create rootfs directory
        std::fs::create_dir_all(&rootfs_path)?;

        // Pull and extract each layer in order
        for (i, layer) in manifest.layers.iter().enumerate() {
            info!(
                "Processing layer {}/{}: {}",
                i + 1,
                manifest.layers.len(),
                layer.digest
            );

            let layer_path = self.pull_layer(image_ref, layer).await?;

            // Extract the layer
            self.extract_layer(&layer_path, &rootfs_path, &layer.media_type)?;
        }

        info!("Rootfs extraction complete: {:?}", rootfs_path);
        Ok(rootfs_path)
    }

    /// Extract layers (stub for when feature is disabled).
    #[cfg(not(feature = "image-pull"))]
    pub async fn extract_layers(
        &self,
        _image_ref: &ImageReference,
        _manifest: &Manifest,
    ) -> Result<PathBuf, ImageError> {
        Err(ImageError::FeatureNotEnabled)
    }

    /// Extract a single layer tarball to the rootfs.
    #[cfg(feature = "image-pull")]
    fn extract_layer(
        &self,
        layer_path: &Path,
        rootfs: &Path,
        media_type: &str,
    ) -> Result<(), ImageError> {
        let file = std::fs::File::open(layer_path)?;

        // Determine if layer is gzipped
        let is_gzipped = media_type.contains("gzip")
            || media_type.contains(".tar.gzip")
            || media_type.ends_with("+gzip");

        if is_gzipped {
            debug!("Extracting gzipped layer");
            let decoder = GzDecoder::new(file);
            let mut archive = Archive::new(decoder);
            self.extract_archive(&mut archive, rootfs)?;
        } else {
            debug!("Extracting uncompressed layer");
            let mut archive = Archive::new(file);
            self.extract_archive(&mut archive, rootfs)?;
        }

        Ok(())
    }

    /// Extract a tar archive, handling OCI whiteouts.
    #[cfg(feature = "image-pull")]
    fn extract_archive<R: io::Read>(
        &self,
        archive: &mut Archive<R>,
        rootfs: &Path,
    ) -> Result<(), ImageError> {
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy().to_string(); // Clone to owned String
            let path_owned = path.to_path_buf(); // Clone for later use

            // Handle OCI whiteout files
            if let Some(filename) = path_owned.file_name() {
                let filename_str = filename.to_string_lossy();

                if filename_str.starts_with(".wh.") {
                    // Whiteout file - delete the corresponding file/directory
                    let target_name = &filename_str[4..]; // Remove ".wh." prefix
                    let target_path = if let Some(parent) = path_owned.parent() {
                        rootfs.join(parent).join(target_name)
                    } else {
                        rootfs.join(target_name)
                    };

                    if target_path.exists() {
                        debug!("Applying whiteout: removing {:?}", target_path);
                        if target_path.is_dir() {
                            std::fs::remove_dir_all(&target_path)?;
                        } else {
                            std::fs::remove_file(&target_path)?;
                        }
                    }
                    continue;
                }

                // Opaque whiteout - clear the directory
                if filename_str == ".wh..wh..opq" {
                    if let Some(parent) = path_owned.parent() {
                        let dir_path = rootfs.join(parent);
                        if dir_path.exists() {
                            debug!("Applying opaque whiteout: clearing {:?}", dir_path);
                            // Remove all contents but keep the directory
                            for dir_entry in std::fs::read_dir(&dir_path)? {
                                let dir_entry = dir_entry?;
                                let entry_path = dir_entry.path();
                                if entry_path.is_dir() {
                                    std::fs::remove_dir_all(&entry_path)?;
                                } else {
                                    std::fs::remove_file(&entry_path)?;
                                }
                            }
                        }
                    }
                    continue;
                }
            }

            // Skip problematic paths
            if path_str.starts_with('/') || path_str.contains("..") {
                warn!("Skipping potentially unsafe path: {}", path_str);
                continue;
            }

            // Extract the entry
            let dest_path = rootfs.join(&path_owned);

            // Create parent directories
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Unpack the entry
            if let Err(e) = entry.unpack_in(rootfs) {
                // Log but don't fail on permission errors for special files
                warn!("Failed to extract {}: {}", path_str, e);
            }
        }

        Ok(())
    }

    /// Convenience method to get a rootfs for an image, pulling if needed.
    #[cfg(feature = "image-pull")]
    pub async fn get_rootfs(&self, image: &str) -> Result<PathBuf, ImageError> {
        let image_ref = Self::parse_image_ref(image)?;

        // Check if already extracted
        let image_id = format!(
            "{}_{}",
            image_ref.repository.replace('/', "_"),
            image_ref.tag
        );
        let rootfs_path = self.cache_dir.join("rootfs").join(&image_id);

        if rootfs_path.exists() {
            info!("Using cached rootfs for {}", image);
            return Ok(rootfs_path);
        }

        // Pull and extract
        let manifest = self.pull_manifest(&image_ref).await?;
        self.extract_layers(&image_ref, &manifest).await
    }

    /// Get rootfs (stub for when feature is disabled).
    #[cfg(not(feature = "image-pull"))]
    pub async fn get_rootfs(&self, _image: &str) -> Result<PathBuf, ImageError> {
        Err(ImageError::FeatureNotEnabled)
    }

    /// List cached images.
    pub fn list_cached(&self) -> Result<Vec<String>, ImageError> {
        let rootfs_dir = self.cache_dir.join("rootfs");
        if !rootfs_dir.exists() {
            return Ok(Vec::new());
        }

        let mut images = Vec::new();
        for entry in std::fs::read_dir(rootfs_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Convert back from filesystem format
                    let image_name = name.replacen('_', "/", 1).replacen('_', ":", 1);
                    images.push(image_name);
                }
            }
        }

        Ok(images)
    }

    /// Remove a cached image.
    pub fn remove_cached(&self, image: &str) -> Result<(), ImageError> {
        let image_ref = Self::parse_image_ref(image)?;
        let image_id = format!(
            "{}_{}",
            image_ref.repository.replace('/', "_"),
            image_ref.tag
        );
        let rootfs_path = self.cache_dir.join("rootfs").join(&image_id);

        if rootfs_path.exists() {
            std::fs::remove_dir_all(&rootfs_path)?;
            info!("Removed cached image: {}", image);
        }

        Ok(())
    }

    /// Clear all cached data.
    pub fn clear_cache(&self) -> Result<(), ImageError> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
            std::fs::create_dir_all(&self.cache_dir)?;
            info!("Cache cleared");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_simple_image() {
        let ref_ = ImageManager::parse_image_ref("alpine").unwrap();
        assert_eq!(ref_.registry, "registry-1.docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert_eq!(ref_.tag, "latest");
    }

    #[test]
    fn test_parse_image_with_tag() {
        let ref_ = ImageManager::parse_image_ref("alpine:3.19").unwrap();
        assert_eq!(ref_.registry, "registry-1.docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert_eq!(ref_.tag, "3.19");
    }

    #[test]
    fn test_parse_user_image() {
        let ref_ = ImageManager::parse_image_ref("myuser/myapp:v1.0").unwrap();
        assert_eq!(ref_.registry, "registry-1.docker.io");
        assert_eq!(ref_.repository, "myuser/myapp");
        assert_eq!(ref_.tag, "v1.0");
    }

    #[test]
    fn test_parse_custom_registry() {
        let ref_ = ImageManager::parse_image_ref("ghcr.io/owner/repo:latest").unwrap();
        assert_eq!(ref_.registry, "ghcr.io");
        assert_eq!(ref_.repository, "owner/repo");
        assert_eq!(ref_.tag, "latest");
    }

    #[test]
    fn test_parse_registry_with_port() {
        let ref_ = ImageManager::parse_image_ref("localhost:5000/myimage").unwrap();
        assert_eq!(ref_.registry, "localhost:5000");
        assert_eq!(ref_.repository, "myimage");
        assert_eq!(ref_.tag, "latest");
    }

    #[test]
    fn test_parse_deep_path() {
        let ref_ = ImageManager::parse_image_ref("gcr.io/my-project/subdir/image:v2").unwrap();
        assert_eq!(ref_.registry, "gcr.io");
        assert_eq!(ref_.repository, "my-project/subdir/image");
        assert_eq!(ref_.tag, "v2");
    }

    #[test]
    fn test_parse_empty_fails() {
        assert!(ImageManager::parse_image_ref("").is_err());
        assert!(ImageManager::parse_image_ref("   ").is_err());
    }

    #[test]
    fn test_image_manager_creation() {
        let temp = TempDir::new().unwrap();
        let manager = ImageManager::new(temp.path().join("cache")).unwrap();
        assert!(manager.cache_dir().exists());
    }

    #[test]
    fn test_list_cached_empty() {
        let temp = TempDir::new().unwrap();
        let manager = ImageManager::new(temp.path().join("cache")).unwrap();
        let cached = manager.list_cached().unwrap();
        assert!(cached.is_empty());
    }

    #[test]
    fn test_image_reference_display() {
        let ref_ = ImageReference::new("registry-1.docker.io", "library/alpine", "latest");
        assert_eq!(
            ref_.to_string(),
            "registry-1.docker.io/library/alpine:latest"
        );
        assert_eq!(ref_.full_name(), "library/alpine:latest");
    }

    #[test]
    fn test_clear_cache() {
        let temp = TempDir::new().unwrap();
        let cache_path = temp.path().join("cache");
        let manager = ImageManager::new(&cache_path).unwrap();

        // Create some files
        std::fs::create_dir_all(cache_path.join("rootfs/test")).unwrap();
        std::fs::write(cache_path.join("rootfs/test/file.txt"), "test").unwrap();

        manager.clear_cache().unwrap();

        // Cache dir should exist but be empty
        assert!(cache_path.exists());
        assert!(std::fs::read_dir(&cache_path).unwrap().next().is_none());
    }
}
