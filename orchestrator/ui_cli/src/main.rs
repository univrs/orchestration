//! AI-Native Orchestrator CLI
//!
//! Command-line interface for managing workloads, nodes, and cluster operations.

mod client;
mod commands;
mod error;
mod output;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::commands::{deploy, init, logs, scale, status};

/// AI-Native Orchestrator CLI
#[derive(Parser)]
#[command(name = "orch")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// API server URL
    #[arg(
        short,
        long,
        env = "ORCH_API_URL",
        default_value = "http://localhost:9090"
    )]
    api_url: String,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize user configuration and identity
    Init(init::InitArgs),

    /// Show cluster and workload status
    Status(status::StatusArgs),

    /// Deploy a new workload
    Deploy(deploy::DeployArgs),

    /// Scale a workload
    Scale(scale::ScaleArgs),

    /// View workload logs
    Logs(logs::LogsArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Execute command
    let result = match cli.command {
        Commands::Init(args) => init::execute(args).await,
        Commands::Status(args) => status::execute(args, &cli.api_url, cli.format).await,
        Commands::Deploy(args) => deploy::execute(args, &cli.api_url, cli.format).await,
        Commands::Scale(args) => scale::execute(args, &cli.api_url, cli.format).await,
        Commands::Logs(args) => logs::execute(args, &cli.api_url).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
