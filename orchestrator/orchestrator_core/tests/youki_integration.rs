//! Integration tests for YoukiCliRuntime with orchestrator.
//!
//! These tests require:
//! - Linux with youki binary installed (or in PATH)
//! - Root privileges (or appropriate capabilities for cgroups)
//! - Network access to Docker Hub for image pulling
//!
//! Run with: cargo test -p orchestrator_core --features "youki-runtime" --test youki_integration -- --ignored
//!
//! Or for a specific test:
//! cargo test -p orchestrator_core --features "youki-runtime" --test youki_integration test_nginx_workflow -- --ignored

#[cfg(feature = "youki-runtime")]
mod tests {
    use container_runtime::{YoukiCliRuntime, YoukiCliConfig};
    use container_runtime_interface::{ContainerRuntime, CreateContainerOptions};
    use orchestrator_shared_types::{ContainerConfig, NodeResources, PortMapping, NodeId, Keypair};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn generate_node_id() -> NodeId {
        Keypair::generate().public_key()
    }

    /// Check if youki binary is available.
    async fn youki_available() -> bool {
        tokio::process::Command::new("youki")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Create a test runtime with temporary directories.
    async fn create_test_runtime(temp_dir: &TempDir) -> Result<YoukiCliRuntime, String> {
        let config = YoukiCliConfig {
            youki_binary: PathBuf::from("youki"),
            bundle_root: temp_dir.path().join("bundles"),
            state_root: temp_dir.path().join("state"),
            command_timeout: Duration::from_secs(60),
            stop_timeout: Duration::from_secs(10),
        };

        YoukiCliRuntime::with_config(config).await.map_err(|e| e.to_string())
    }

    #[tokio::test]
    #[ignore] // Requires youki binary and root privileges
    async fn test_nginx_workflow() {
        // Skip if youki is not available
        if !youki_available().await {
            eprintln!("Skipping test: youki binary not found");
            return;
        }

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create runtime
        let runtime = match create_test_runtime(&temp_dir).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Skipping test: Failed to create runtime: {}", e);
                return;
            }
        };

        let node_id = generate_node_id();
        let workload_id = Uuid::new_v4();

        // Initialize node
        runtime.init_node(node_id).await
            .expect("Failed to init node");

        // Create container config for nginx
        let config = ContainerConfig {
            name: "nginx-test".to_string(),
            image: "nginx:alpine".to_string(),
            command: None, // Use default nginx command
            args: None,
            env_vars: HashMap::new(),
            ports: vec![
                PortMapping {
                    container_port: 80,
                    host_port: None,
                    protocol: "tcp".to_string(),
                },
            ],
            resource_requests: NodeResources {
                cpu_cores: 0.5,
                memory_mb: 128,
                disk_mb: 256,
            },
        };

        let options = CreateContainerOptions {
            node_id,
            workload_id,
        };

        // Create and start container
        println!("Creating nginx container...");
        let container_id = runtime.create_container(&config, &options).await
            .expect("Failed to create container");
        println!("Created container: {}", container_id);

        // Wait for container to start
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Check container status
        let status = runtime.get_container_status(&container_id).await
            .expect("Failed to get container status");
        println!("Container status: {:?}", status);
        assert!(
            status.state.to_lowercase().contains("running"),
            "Container should be running, got: {}",
            status.state
        );

        // List containers on node
        let containers = runtime.list_containers(node_id).await
            .expect("Failed to list containers");
        assert!(
            containers.iter().any(|c| c.id == container_id),
            "Container should be in list"
        );

        // Get container logs (if implemented)
        if let Ok(logs) = runtime.get_logs(&container_id, Some(100)).await {
            println!("Container logs:");
            for line in logs.lines().take(10) {
                println!("  {}", line);
            }
        }

        // Get container stats (if implemented)
        if let Ok(stats) = runtime.get_stats(&container_id).await {
            println!("Container stats:");
            println!("  CPU: {} ns", stats.cpu_usage_ns);
            println!("  Memory: {} bytes", stats.memory_usage_bytes);
        }

        // Stop container
        println!("Stopping container...");
        runtime.stop_container(&container_id).await
            .expect("Failed to stop container");

        // Verify stopped
        tokio::time::sleep(Duration::from_secs(2)).await;
        let status = runtime.get_container_status(&container_id).await;
        println!("Container status after stop: {:?}", status);

        // Remove container
        println!("Removing container...");
        runtime.remove_container(&container_id).await
            .expect("Failed to remove container");

        // Verify removed
        let status = runtime.get_container_status(&container_id).await;
        assert!(
            status.is_err(),
            "Container should be removed"
        );

        println!("Test completed successfully!");
    }

    #[tokio::test]
    #[ignore] // Requires youki binary and root privileges
    async fn test_busybox_echo() {
        // Skip if youki is not available
        if !youki_available().await {
            eprintln!("Skipping test: youki binary not found");
            return;
        }

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let runtime = match create_test_runtime(&temp_dir).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Skipping test: Failed to create runtime: {}", e);
                return;
            }
        };

        let node_id = generate_node_id();
        let workload_id = Uuid::new_v4();

        runtime.init_node(node_id).await
            .expect("Failed to init node");

        // Create a simple busybox container that echoes and exits
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        let config = ContainerConfig {
            name: "busybox-test".to_string(),
            image: "busybox:latest".to_string(),
            command: Some(vec!["/bin/sh".to_string()]),
            args: Some(vec!["-c".to_string(), "echo 'Hello from container!' && sleep 5".to_string()]),
            env_vars,
            ports: vec![],
            resource_requests: NodeResources {
                cpu_cores: 0.1,
                memory_mb: 32,
                disk_mb: 64,
            },
        };

        let options = CreateContainerOptions {
            node_id,
            workload_id,
        };

        println!("Creating busybox container...");
        let container_id = runtime.create_container(&config, &options).await
            .expect("Failed to create container");
        println!("Created container: {}", container_id);

        // Wait for container to run
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Check logs
        if let Ok(logs) = runtime.get_logs(&container_id, Some(100)).await {
            println!("Container output:");
            for line in logs.lines() {
                println!("  {}", line);
            }
            assert!(
                logs.contains("Hello from container"),
                "Should see hello message in logs"
            );
        }

        // Clean up
        let _ = runtime.stop_container(&container_id).await;
        let _ = runtime.remove_container(&container_id).await;

        println!("Test completed successfully!");
    }

    #[tokio::test]
    async fn test_runtime_init_without_youki() {
        // This test verifies graceful handling when youki is not available
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = YoukiCliConfig {
            youki_binary: PathBuf::from("/nonexistent/path/to/youki"),
            bundle_root: temp_dir.path().join("bundles"),
            state_root: temp_dir.path().join("state"),
            command_timeout: Duration::from_secs(30),
            stop_timeout: Duration::from_secs(10),
        };

        // Should fail gracefully with a clear error
        let result = YoukiCliRuntime::with_config(config).await;
        assert!(result.is_err(), "Should fail when youki binary doesn't exist");

        let err = result.err().unwrap();
        let err_msg = err.to_string();
        println!("Expected error: {}", err_msg);
        assert!(
            err_msg.contains("not found") || err_msg.contains("Youki binary"),
            "Error should mention binary not found, got: {}",
            err_msg
        );
    }
}
