//! Target command handler
//!
//! Handles `cx target` subcommands for managing cross-compilation targets.

use anyhow::{Result, anyhow};
use colored::*;
use std::path::Path;

const TARGET_DEFERRED_REASON: &str =
    "Target mutation commands are deferred in v0.3.x patch releases";

/// Target subcommand operations
#[derive(Clone, Debug)]
pub enum TargetOp {
    /// List all available targets
    List,
    /// Add a target to the project
    Add { name: String },
    /// Remove a target from the project
    Remove { name: String },
    /// Set the default target
    Default { name: String },
}

/// Handle the `cx target` command for cross-compilation targets
pub fn handle_target_command(op: &Option<TargetOp>) -> Result<()> {
    let config_path = Path::new("cx.toml");

    match op {
        None | Some(TargetOp::List) => render_target_list(config_path),
        Some(TargetOp::Add { name }) => return deferred_target_mutation("add", name),
        Some(TargetOp::Remove { name }) => return deferred_target_mutation("remove", name),
        Some(TargetOp::Default { name }) => return deferred_target_mutation("default", name),
    }

    Ok(())
}

fn render_target_list(config_path: &Path) {
    print_target_header();
    print_target_catalog();
    print_profile_configuration_status(config_path);
    print_target_usage_hint();
}

fn print_target_header() {
    println!(
        "{} {}",
        "🎯".cyan(),
        "Available Cross-Compilation Targets".bold()
    );
    println!("{}", "─".repeat(50).dimmed());
    println!();
    println!(
        "{} Target mutation commands ({}, {}, {}) are deferred in v0.3.x patch releases.",
        "ℹ".blue(),
        "add".yellow(),
        "remove".yellow(),
        "default".yellow()
    );
    println!(
        "   Configure cross targets with {} and run {}.",
        "[profile:<name>]".cyan(),
        "cx build --profile <name>".cyan()
    );
    println!();
}

fn print_target_catalog() {
    println!(
        "   {} (MSVC) - Windows 64-bit",
        "windows-x64".green().bold()
    );
    println!(
        "   {} (MinGW) - Windows 64-bit GNU",
        "windows-x64-gnu".green()
    );
    println!("   {} (GCC/Clang) - Linux 64-bit", "linux-x64".blue());
    println!("   {} (Cross) - Linux ARM64", "linux-arm64".blue());
    println!("   {} (Clang) - macOS Intel", "macos-x64".magenta());
    println!(
        "   {} (Clang) - macOS Apple Silicon",
        "macos-arm64".magenta()
    );
    println!("   {} (Emscripten) - WebAssembly", "wasm32".yellow());
    println!("   {} (ESP-IDF) - ESP32 Microcontroller", "esp32".red());
}

fn print_profile_configuration_status(config_path: &Path) {
    println!();
    if config_path.exists()
        && let Ok(content) = std::fs::read_to_string(config_path)
    {
        if content.contains("[profile:") {
            println!(
                "{} Profile-based target configuration detected",
                "✓".green()
            );
        } else {
            println!(
                "{} No target profiles configured. Add {} and run {}.",
                "!".yellow(),
                "[profile:<name>]".cyan(),
                "cx build --profile <name>".cyan()
            );
        }
    }
}

fn print_target_usage_hint() {
    println!();
    println!("Usage: {}", "cx target list".cyan());
    println!(
        "Deferred: {}, {}, {}",
        "cx target add <name>".dimmed(),
        "cx target remove <name>".dimmed(),
        "cx target default <name>".dimmed()
    );
    println!(
        "Hint: configure cross builds with {} and run {}",
        "[profile:<name>]".cyan(),
        "cx build --profile <name>".cyan()
    );
}

fn deferred_target_mutation(operation: &str, target_name: &str) -> Result<()> {
    eprintln!(
        "{} '{}' is deferred in v0.3.x patch releases.",
        "x".red(),
        format!("cx target {operation} {target_name}").yellow()
    );
    eprintln!(
        "  Configure targets via {} and run {}.",
        "[profile:<name>]".cyan(),
        "cx build --profile <name>".cyan()
    );
    Err(anyhow!(TARGET_DEFERRED_REASON))
}
