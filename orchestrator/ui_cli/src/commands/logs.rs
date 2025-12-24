//! Logs command - view workload logs.

#![allow(dead_code)]

use clap::Args;
use colored::Colorize;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client::ApiClient;
use crate::error::{CliError, Result};
use crate::output;

/// Arguments for the logs command.
#[derive(Args)]
pub struct LogsArgs {
    /// Workload ID or name
    workload: String,

    /// Follow logs (stream new entries)
    #[arg(short, long)]
    follow: bool,

    /// Number of lines to show (tail)
    #[arg(short = 'n', long, default_value = "100")]
    tail: usize,

    /// Show timestamps
    #[arg(short, long)]
    timestamps: bool,

    /// Filter logs since timestamp (RFC3339)
    #[arg(long)]
    since: Option<String>,

    /// Filter logs until timestamp (RFC3339)
    #[arg(long)]
    until: Option<String>,

    /// Filter by instance ID
    #[arg(short, long)]
    instance: Option<String>,

    /// Filter by container name
    #[arg(short, long)]
    container: Option<String>,
}

/// List workloads response.
#[derive(Debug, Deserialize)]
struct ListWorkloadsResponse {
    items: Vec<WorkloadResponse>,
}

/// Workload response for finding by name.
#[derive(Debug, Deserialize)]
struct WorkloadResponse {
    id: String,
    name: String,
}

/// Instance response.
#[derive(Debug, Deserialize)]
struct InstanceResponse {
    id: String,
    node_id: String,
    status: String,
    container_ids: Vec<String>,
}

/// Instances list response.
#[derive(Debug, Deserialize)]
struct ListInstancesResponse {
    items: Vec<InstanceResponse>,
}

/// Log response from REST API.
#[derive(Debug, Deserialize)]
struct LogsResponse {
    workload_id: String,
    instance_id: Option<String>,
    container_id: Option<String>,
    logs: String,
    lines: usize,
}

/// WebSocket log message.
#[derive(Debug, Deserialize)]
struct WsLogMessage {
    container_id: Option<String>,
    instance_id: Option<String>,
    log: Option<String>,
    error: Option<String>,
    status: Option<String>,
    info: Option<String>,
}

/// Execute the logs command.
pub async fn execute(args: LogsArgs, api_url: &str) -> anyhow::Result<()> {
    let client = match ApiClient::authenticated(api_url).await {
        Ok(c) => c,
        Err(e) => {
            output::warn(&format!("Could not load identity: {}", e));
            ApiClient::new(api_url)
        }
    };

    // Find the workload
    let workload_id = find_workload_id(&client, &args.workload).await?;

    if args.follow {
        // WebSocket streaming mode
        stream_logs(api_url, &workload_id, &args).await
    } else {
        // REST API fetch mode
        fetch_logs(&client, &workload_id, &args).await
    }
}

/// Fetch logs via REST API.
async fn fetch_logs(client: &ApiClient, workload_id: &str, args: &LogsArgs) -> anyhow::Result<()> {
    output::info(&format!("Fetching logs for workload '{}'...", args.workload));

    // Build query parameters
    let mut query_params = vec![format!("tail={}", args.tail)];

    if args.timestamps {
        query_params.push("timestamps=true".to_string());
    }

    if let Some(ref since) = args.since {
        query_params.push(format!("since={}", since));
    }

    if let Some(ref until) = args.until {
        query_params.push(format!("until={}", until));
    }

    let query_string = if query_params.is_empty() {
        String::new()
    } else {
        format!("?{}", query_params.join("&"))
    };

    // Fetch logs - either for specific instance or all instances
    let response: LogsResponse = if let Some(ref instance_id) = args.instance {
        let path = format!(
            "/api/v1/workloads/{}/instances/{}/logs{}",
            workload_id, instance_id, query_string
        );
        client.get(&path).await?
    } else {
        let path = format!("/api/v1/workloads/{}/logs{}", workload_id, query_string);
        client.get(&path).await?
    };

    // Display logs
    if response.logs.is_empty() {
        output::warn("No logs found for this workload");
        return Ok(());
    }

    println!();

    // Apply container filter if specified
    for section in response.logs.split("\n\n") {
        if section.is_empty() {
            continue;
        }

        // Check for container header
        if let Some(first_line) = section.lines().next() {
            if first_line.starts_with("=== Container:") {
                // Extract container ID
                let container_id = first_line
                    .trim_start_matches("=== Container: ")
                    .trim_end_matches(" ===");

                // Apply container filter
                if let Some(ref filter) = args.container {
                    if !container_id.contains(filter) {
                        continue;
                    }
                }

                // Print header
                println!("{}", first_line.cyan());

                // Print log lines
                for line in section.lines().skip(1) {
                    print_log_line(line, args.timestamps);
                }
                println!();
            } else {
                // No header, print as-is
                for line in section.lines() {
                    print_log_line(line, args.timestamps);
                }
            }
        }
    }

    output::info(&format!("Showing {} line(s)", response.lines));
    Ok(())
}

/// Print a single log line with optional formatting.
fn print_log_line(line: &str, show_timestamps: bool) {
    if line.is_empty() {
        return;
    }

    // Try to parse timestamp from line
    let parts: Vec<&str> = line.splitn(2, ' ').collect();

    if parts.len() == 2 && parts[0].contains('T') && parts[0].len() > 10 {
        // Looks like a timestamped line
        if show_timestamps {
            println!("{} {}", parts[0].dimmed(), parts[1]);
        } else {
            println!("{}", parts[1]);
        }
    } else {
        // No timestamp or timestamps not requested
        println!("{}", line);
    }
}

/// Stream logs via WebSocket.
async fn stream_logs(api_url: &str, workload_id: &str, args: &LogsArgs) -> anyhow::Result<()> {
    output::info(&format!("Streaming logs for workload '{}' (Ctrl+C to stop)...", args.workload));

    // Convert HTTP URL to WebSocket URL
    let ws_url = api_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");

    let ws_endpoint = format!("{}/api/v1/workloads/{}/logs/stream", ws_url, workload_id);

    // Connect to WebSocket
    let (ws_stream, _) = connect_async(&ws_endpoint).await.map_err(|e| {
        CliError::api_error(format!("Failed to connect to WebSocket: {}", e))
    })?;

    output::info(&format!("Connected to {}", ws_endpoint));
    println!();

    let (_, mut read) = ws_stream.split();

    // Read messages
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<WsLogMessage>(&text) {
                    Ok(log_msg) => {
                        // Handle different message types
                        if let Some(error) = log_msg.error {
                            output::error(&error);
                            break;
                        }

                        if let Some(status) = log_msg.status {
                            output::info(&format!("Status: {}", status));
                            continue;
                        }

                        if let Some(info) = log_msg.info {
                            output::info(&info);
                            continue;
                        }

                        // Log message
                        if let Some(log) = log_msg.log {
                            // Apply container filter
                            if let Some(ref filter) = args.container {
                                if let Some(ref cid) = log_msg.container_id {
                                    if !cid.contains(filter) {
                                        continue;
                                    }
                                }
                            }

                            // Apply instance filter
                            if let Some(ref filter) = args.instance {
                                if let Some(ref iid) = log_msg.instance_id {
                                    if !iid.starts_with(filter) {
                                        continue;
                                    }
                                }
                            }

                            // Format output
                            let container_prefix = log_msg
                                .container_id
                                .as_ref()
                                .map(|c| format!("[{}]", &c[..12.min(c.len())]).cyan().to_string())
                                .unwrap_or_default();

                            print_log_line(&format!("{} {}", container_prefix, log), args.timestamps);
                        }
                    }
                    Err(_) => {
                        // Try raw JSON display for debugging
                        tracing::debug!("Received: {}", text);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                output::info("Connection closed by server");
                break;
            }
            Ok(Message::Ping(_)) => {
                // Ping handled automatically by tungstenite
            }
            Err(e) => {
                output::error(&format!("WebSocket error: {}", e));
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Find workload ID by name or ID.
async fn find_workload_id(client: &ApiClient, name_or_id: &str) -> Result<String> {
    // First try as UUID directly
    if uuid::Uuid::parse_str(name_or_id).is_ok() {
        return Ok(name_or_id.to_string());
    }

    // Otherwise search by name
    let response: ListWorkloadsResponse = client.get("/api/v1/workloads").await?;

    let matching: Vec<_> = response.items
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
