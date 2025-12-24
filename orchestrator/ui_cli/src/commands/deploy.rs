//! Deploy command - deploy a new workload.

use clap::Args;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::{self, print_item};
use crate::OutputFormat;

/// Arguments for the deploy command.
#[derive(Args)]
pub struct DeployArgs {
    /// Workload name
    #[arg(short, long)]
    name: String,

    /// Container image
    #[arg(short, long)]
    image: String,

    /// Number of replicas
    #[arg(short, long, default_value = "1")]
    replicas: u32,

    /// Container ports (format: CONTAINER_PORT or CONTAINER_PORT:HOST_PORT, can be repeated)
    /// Examples: --port 80 --port 443:8443
    #[arg(short, long, value_parser = parse_port_mapping)]
    port: Vec<PortSpec>,

    /// CPU request in millicores (e.g., 100 = 0.1 cores, 1000 = 1 core)
    #[arg(long, default_value = "100")]
    cpu: u64,

    /// Memory request in MB
    #[arg(long, default_value = "128")]
    memory: u64,

    /// Disk request in MB (optional)
    #[arg(long, default_value = "0")]
    disk: u64,

    /// Environment variables (KEY=VALUE format, can be repeated)
    /// Example: --env DB_HOST=localhost --env DB_PORT=5432
    #[arg(short, long, value_parser = parse_env_var)]
    env: Vec<(String, String)>,

    /// Labels (key=value format, can be repeated)
    /// Example: --label app=nginx --label tier=frontend
    #[arg(short, long, value_parser = parse_env_var)]
    label: Vec<(String, String)>,
}

/// Parsed port specification.
#[derive(Debug, Clone)]
pub struct PortSpec {
    pub container_port: u16,
    pub host_port: Option<u16>,
}

fn parse_port_mapping(s: &str) -> std::result::Result<PortSpec, String> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            let container_port = parts[0]
                .parse::<u16>()
                .map_err(|_| format!("Invalid port number: {}", parts[0]))?;
            Ok(PortSpec {
                container_port,
                host_port: None,
            })
        }
        2 => {
            let container_port = parts[0]
                .parse::<u16>()
                .map_err(|_| format!("Invalid container port: {}", parts[0]))?;
            let host_port = parts[1]
                .parse::<u16>()
                .map_err(|_| format!("Invalid host port: {}", parts[1]))?;
            Ok(PortSpec {
                container_port,
                host_port: Some(host_port),
            })
        }
        _ => Err(format!(
            "Invalid port format '{}'. Use CONTAINER_PORT or CONTAINER_PORT:HOST_PORT",
            s
        )),
    }
}

fn parse_env_var(s: &str) -> std::result::Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid format '{}', expected KEY=VALUE", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Create workload request - matches API's CreateWorkloadRequest.
#[derive(Debug, Serialize)]
struct CreateWorkloadRequest {
    name: String,
    replicas: u32,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
    containers: Vec<ContainerConfigRequest>,
}

/// Container configuration - matches API's ContainerConfigRequest.
#[derive(Debug, Serialize)]
struct ContainerConfigRequest {
    name: String,
    image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<Vec<String>>,
    #[serde(default)]
    env_vars: std::collections::HashMap<String, String>,
    #[serde(default)]
    ports: Vec<PortMappingRequest>,
    #[serde(default)]
    resource_requests: ResourceRequestsRequest,
}

/// Port mapping - matches API's PortMappingRequest.
#[derive(Debug, Serialize)]
struct PortMappingRequest {
    container_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_port: Option<u16>,
    protocol: String,
}

/// Resource requests - matches API's ResourceRequestsRequest.
#[derive(Debug, Serialize, Default)]
struct ResourceRequestsRequest {
    #[serde(default)]
    cpu_cores: f32,
    #[serde(default)]
    memory_mb: u64,
    #[serde(default)]
    disk_mb: u64,
}

/// Workload response from API.
#[derive(Debug, Serialize, Deserialize, Tabled)]
struct WorkloadResponse {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Replicas")]
    replicas: u32,
}

/// Execute the deploy command.
pub async fn execute(args: DeployArgs, api_url: &str, format: OutputFormat) -> anyhow::Result<()> {
    let client = ApiClient::authenticated(api_url).await.map_err(|e| {
        CliError::config_error(format!(
            "Authentication required for deploy. Run 'orch init' first. Error: {}",
            e
        ))
    })?;

    output::info(&format!("Deploying workload '{}'...", args.name));

    // Build the request
    let mut labels = std::collections::HashMap::new();
    for (key, value) in args.label {
        labels.insert(key, value);
    }

    let mut env = std::collections::HashMap::new();
    for (key, value) in args.env {
        env.insert(key, value);
    }

    // Convert port specs to API format
    let ports: Vec<PortMappingRequest> = args
        .port
        .iter()
        .map(|p| PortMappingRequest {
            container_port: p.container_port,
            host_port: p.host_port,
            protocol: "tcp".to_string(),
        })
        .collect();

    // Save counts for status message before moving values
    let port_count = ports.len();
    let env_count = env.len();

    // Build port strings for status message
    let port_strs: Vec<String> = ports
        .iter()
        .map(|p| match p.host_port {
            Some(hp) => format!("{}:{}", p.container_port, hp),
            None => p.container_port.to_string(),
        })
        .collect();

    // Convert CPU from millicores to cores (e.g., 100 millicores = 0.1 cores)
    let cpu_cores = args.cpu as f32 / 1000.0;

    let request = CreateWorkloadRequest {
        name: args.name.clone(),
        replicas: args.replicas,
        labels,
        containers: vec![ContainerConfigRequest {
            name: args.name.clone(),
            image: args.image.clone(),
            command: None,
            args: None,
            env_vars: env,
            ports,
            resource_requests: ResourceRequestsRequest {
                cpu_cores,
                memory_mb: args.memory,
                disk_mb: args.disk,
            },
        }],
    };

    // Send the request
    let response: WorkloadResponse = client.post("/api/v1/workloads", &request).await?;

    output::success(&format!("Workload '{}' deployed successfully!", args.name));
    print_item(&response, format)?;

    // Build informative status message
    let mut info_parts = vec![format!(
        "Scheduling {} replica(s) with image '{}'",
        args.replicas, args.image
    )];

    if port_count > 0 {
        info_parts.push(format!("Exposing ports: {}", port_strs.join(", ")));
    }

    if env_count > 0 {
        info_parts.push(format!("With {} environment variable(s)", env_count));
    }

    for part in info_parts {
        output::info(&part);
    }
    output::info("Use 'orch status' to check deployment progress");

    Ok(())
}
