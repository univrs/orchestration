//! Scale command - scale a workload.

use clap::Args;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::client::ApiClient;
use crate::error::{CliError, Result};
use crate::output::{self, print_item};
use crate::OutputFormat;

/// Arguments for the scale command.
#[derive(Args)]
pub struct ScaleArgs {
    /// Workload ID or name
    workload: String,

    /// Target number of replicas
    #[arg(short, long)]
    replicas: u32,
}

/// Update workload request (partial update).
#[derive(Debug, Serialize)]
struct UpdateWorkloadRequest {
    replicas: u32,
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

/// Execute the scale command.
pub async fn execute(args: ScaleArgs, api_url: &str, format: OutputFormat) -> anyhow::Result<()> {
    let client = ApiClient::authenticated(api_url).await.map_err(|e| {
        CliError::config_error(format!(
            "Authentication required for scale. Run 'orch init' first. Error: {}",
            e
        ))
    })?;

    // First, try to find the workload by ID or name
    let workload_id = find_workload_id(&client, &args.workload).await?;

    output::info(&format!(
        "Scaling workload '{}' to {} replica(s)...",
        args.workload, args.replicas
    ));

    let request = UpdateWorkloadRequest {
        replicas: args.replicas,
    };

    let path = format!("/api/v1/workloads/{}", workload_id);
    let response: WorkloadResponse = client.put(&path, &request).await?;

    output::success(&format!(
        "Workload scaled to {} replica(s)",
        response.replicas
    ));
    print_item(&response, format)?;

    output::info("Use 'orch status' to check scaling progress");

    Ok(())
}

/// Find workload ID by name or ID.
async fn find_workload_id(client: &ApiClient, name_or_id: &str) -> Result<String> {
    // First try as UUID directly
    if uuid::Uuid::parse_str(name_or_id).is_ok() {
        return Ok(name_or_id.to_string());
    }

    // Otherwise search by name
    let workloads: Vec<WorkloadResponse> = client.get("/api/v1/workloads").await?;

    let matching: Vec<_> = workloads
        .iter()
        .filter(|w| w.name == name_or_id || w.id.starts_with(name_or_id))
        .collect();

    match matching.len() {
        0 => Err(CliError::WorkloadNotFound(name_or_id.to_string())),
        1 => Ok(matching[0].id.clone()),
        _ => Err(CliError::invalid_argument(format!(
            "Ambiguous workload reference '{}', matches {} workloads. Use full ID.",
            name_or_id,
            matching.len()
        ))),
    }
}
