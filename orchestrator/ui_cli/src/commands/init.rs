//! Initialize user configuration and identity.

use clap::Args;
use colored::Colorize;
use user_config::{ConfigPaths, UserConfig};

use crate::output;

/// Arguments for the init command.
#[derive(Args)]
pub struct InitArgs {
    /// Force re-initialization even if config exists
    #[arg(short, long)]
    force: bool,

    /// Display name for the identity
    #[arg(short, long)]
    name: Option<String>,

    /// Email for the identity
    #[arg(short, long)]
    email: Option<String>,
}

/// Execute the init command.
pub async fn execute(args: InitArgs) -> anyhow::Result<()> {
    let paths = ConfigPaths::new()?;

    // Check if config already exists
    if paths.identity_file().exists() && !args.force {
        output::warn("Configuration already exists. Use --force to reinitialize.");
        output::info(&format!(
            "Config directory: {}",
            paths.config_dir().display()
        ));

        // Load and display existing identity
        let config = UserConfig::load_or_create().await?;
        let identity = config.identity();

        output::section("Current Identity");
        println!("  {} {}", "ID:".dimmed(), identity.id());
        println!(
            "  {} {}",
            "Public Key:".dimmed(),
            identity.public_key_base64()
        );
        if let Some(name) = identity.display_name() {
            println!("  {} {}", "Name:".dimmed(), name);
        }
        if let Some(email) = identity.email() {
            println!("  {} {}", "Email:".dimmed(), email);
        }
        println!(
            "  {} {}",
            "Created:".dimmed(),
            identity.created_at().format("%Y-%m-%d %H:%M:%S UTC")
        );

        return Ok(());
    }

    output::info("Initializing orchestrator configuration...");

    // Create directories
    paths.ensure_dirs().await?;
    output::success(&format!(
        "Created config directory: {}",
        paths.config_dir().display()
    ));

    // Create or recreate the configuration
    let config = if args.force && paths.identity_file().exists() {
        // Delete existing and create new
        std::fs::remove_file(paths.identity_file())?;
        output::info("Removed existing identity, generating new one...");
        UserConfig::load_or_create().await?
    } else {
        UserConfig::load_or_create().await?
    };

    // Update identity with provided name/email if specified
    let identity = config.identity();

    output::success("Identity generated successfully!");
    output::section("Identity Details");
    println!("  {} {}", "ID:".dimmed(), identity.id());
    println!(
        "  {} {}",
        "Public Key:".dimmed(),
        identity.public_key_base64()
    );
    if let Some(name) = identity.display_name().or(args.name.as_deref()) {
        println!("  {} {}", "Name:".dimmed(), name);
    }
    if let Some(email) = identity.email().or(args.email.as_deref()) {
        println!("  {} {}", "Email:".dimmed(), email);
    }
    println!(
        "  {} {}",
        "Created:".dimmed(),
        identity.created_at().format("%Y-%m-%d %H:%M:%S UTC")
    );

    output::section("Configuration Files");
    println!(
        "  {} {}",
        "Identity:".dimmed(),
        paths.identity_file().display()
    );
    println!(
        "  {} {}",
        "Trust Policy:".dimmed(),
        paths.trust_policy_file().display()
    );
    println!(
        "  {} {}",
        "Secrets:".dimmed(),
        paths.secrets_dir().display()
    );

    output::section("Next Steps");
    println!("  1. Share your public key with cluster administrators");
    println!("  2. Configure trust policy: {}", paths.trust_policy_file().display());
    println!("  3. Connect to cluster: orch status --api-url <URL>");

    Ok(())
}
