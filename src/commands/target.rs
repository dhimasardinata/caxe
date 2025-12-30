//! Target command handler
//!
//! Handles `cx target` subcommands for managing cross-compilation targets.

use anyhow::Result;
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
            println!(
                "Usage: {} or {}",
                "cx target add <name>".cyan(),
                "cx build --target <name>".cyan()
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

            let valid_targets = [
                "windows-x64",
                "windows-x64-gnu",
                "linux-x64",
                "linux-arm64",
                "macos-x64",
                "macos-arm64",
                "wasm32",
                "esp32",
            ];

            if !valid_targets.contains(&name.as_str()) {
                println!(
                    "{} Unknown target '{}'. Run {} to see available targets.",
                    "x".red(),
                    name,
                    "cx target list".cyan()
                );
                return Ok(());
            }

            // Read and update config
            let mut content = std::fs::read_to_string(config_path)?;

            if content.contains(&format!("\"{}\"", name)) {
                println!("{} Target '{}' already configured.", "!".yellow(), name);
                return Ok(());
            }

            // Add targets section if not present
            if !content.contains("[targets]") {
                content.push_str(&format!("\n[targets]\nlist = [\"{}\"]\n", name));
            } else {
                // Append to existing targets list
                content = content.replace("list = [", &format!("list = [\"{}\", ", name));
            }

            std::fs::write(config_path, content)?;
            println!("{} Added target: {}", "✓".green(), name.cyan());
            println!(
                "   Build with: {}",
                format!("cx build --target {}", name).yellow()
            );
        }
        Some(TargetOp::Remove { name }) => {
            if !config_path.exists() {
                println!("{} No cx.toml found.", "x".red());
                return Ok(());
            }

            let content = std::fs::read_to_string(config_path)?;
            let new_content = content
                .replace(&format!("\"{}\", ", name), "")
                .replace(&format!(", \"{}\"", name), "")
                .replace(&format!("\"{}\"", name), "");

            std::fs::write(config_path, new_content)?;
            println!("{} Removed target: {}", "✓".green(), name);
        }
        Some(TargetOp::Default { name }) => {
            if !config_path.exists() {
                println!("{} No cx.toml found.", "x".red());
                return Ok(());
            }

            let mut content = std::fs::read_to_string(config_path)?;

            // Add or update default_target
            if content.contains("default_target") {
                // Replace existing
                let re = {
                    use std::sync::OnceLock;
                    static RE: OnceLock<regex::Regex> = OnceLock::new();
                    RE.get_or_init(|| regex::Regex::new(r#"default_target\s*=\s*"[^"]*""#).unwrap())
                };
                content = re
                    .replace(&content, &format!("default_target = \"{}\"", name))
                    .to_string();
            } else if content.contains("[targets]") {
                content = content.replace(
                    "[targets]",
                    &format!("[targets]\ndefault_target = \"{}\"", name),
                );
            } else {
                content.push_str(&format!("\n[targets]\ndefault_target = \"{}\"\n", name));
            }

            std::fs::write(config_path, content)?;
            println!("{} Set default target: {}", "✓".green(), name.cyan());
        }
    }
    Ok(())
}
