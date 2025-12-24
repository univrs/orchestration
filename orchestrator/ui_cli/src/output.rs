//! Output formatting utilities.

use colored::Colorize;
use serde::Serialize;
use tabled::{Table, Tabled};

use crate::OutputFormat;

/// Print data in the specified format.
pub fn print_data<T: Serialize + Tabled>(data: &[T], format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Table => {
            if data.is_empty() {
                println!("{}", "No items found.".dimmed());
            } else {
                let table = Table::new(data);
                println!("{}", table);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml_ng::to_string(data)?);
        }
    }
    Ok(())
}

/// Print a single item in the specified format.
pub fn print_item<T: Serialize + Tabled>(item: &T, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Table => {
            let table = Table::new([item]);
            println!("{}", table);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(item)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml_ng::to_string(item)?);
        }
    }
    Ok(())
}

/// Print a success message.
pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

/// Print an info message.
pub fn info(msg: &str) {
    println!("{} {}", "→".blue(), msg);
}

/// Print a warning message.
pub fn warn(msg: &str) {
    println!("{} {}", "!".yellow().bold(), msg);
}

/// Print an error message.
pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

/// Print a section header.
pub fn section(title: &str) {
    println!("\n{}", title.bold().underline());
}
