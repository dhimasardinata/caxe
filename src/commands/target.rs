//! Target command handler
//!
//! Handles `cx target` subcommands for managing cross-compilation targets.

use anyhow::{Result, anyhow};
use colored::*;
use std::path::Path;

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
        None | Some(TargetOp::List) => {
            println!(
                "{} {}",
                "🎯".cyan(),
                "Available Cross-Compilation Targets".bold()
            );
            println!("{}", "─".repeat(50).dimmed());
            println!();
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
            println!();

            // Show configured targets if in a project
            if config_path.exists()
                && let Ok(content) = std::fs::read_to_string(config_path)
            {
                if content.contains("[targets]") || content.contains("targets =") {
                    println!("{} Project targets configured", "✓".green());
                } else {
                    println!(
                        "{} No targets configured. Use {} to add one.",
                        "!".yellow(),
                        "cx target add <name>".cyan()
                    );
                }
            }
            println!();
            println!("Usage: {}", "cx target add <name>".cyan());
            println!(
                "Hint: configure cross builds with {} and run {}",
                "[profile:<name>]".cyan(),
                "cx build --profile <name>".cyan()
            );
        }
        Some(TargetOp::Add { name }) => {
            if !config_path.exists() {
                println!(
                    "{} No cx.toml found. Run {} first.",
                    "x".red(),
                    "cx init".cyan()
                );
                return Ok(());
            }
            eprintln!(
                "{} '{}' is deferred. Configure targets via {} and run {}.",
                "x".red(),
                format!("cx target add {}", name).yellow(),
                "[profile:<name>]".cyan(),
                "cx build --profile <name>".cyan()
            );
            return Err(anyhow!(
                "Target mutation commands are deferred in this release"
            ));
        }
        Some(TargetOp::Remove { name }) => {
            if !config_path.exists() {
                println!("{} No cx.toml found.", "x".red());
                return Ok(());
            }
            eprintln!(
                "{} '{}' is deferred. Configure targets via {} and run {}.",
                "x".red(),
                format!("cx target remove {}", name).yellow(),
                "[profile:<name>]".cyan(),
                "cx build --profile <name>".cyan()
            );
            return Err(anyhow!(
                "Target mutation commands are deferred in this release"
            ));
        }
        Some(TargetOp::Default { name }) => {
            if !config_path.exists() {
                println!("{} No cx.toml found.", "x".red());
                return Ok(());
            }
            eprintln!(
                "{} '{}' is deferred. Configure targets via {} and run {}.",
                "x".red(),
                format!("cx target default {}", name).yellow(),
                "[profile:<name>]".cyan(),
                "cx build --profile <name>".cyan()
            );
            return Err(anyhow!(
                "Target mutation commands are deferred in this release"
            ));
        }
    }
    Ok(())
}
