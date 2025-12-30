//! Framework command handler
//!
//! Handles `cx framework` subcommands for managing C++ frameworks like daxe.

use anyhow::Result;
use colored::*;
use inquire::Select;

use crate::ui;

/// Built-in frameworks with their Git URLs
pub const FRAMEWORKS: &[(&str, &str, &str)] = &[
    (
        "daxe",
        "https://github.com/dhimasardinata/daxe.git",
        "D.A's Axe - Cut through C++ verbosity",
    ),
    (
        "fmt",
        "https://github.com/fmtlib/fmt.git",
        "Modern formatting library",
    ),
    (
        "spdlog",
        "https://github.com/gabime/spdlog.git",
        "Fast C++ logging library",
    ),
    (
        "json",
        "https://github.com/nlohmann/json.git",
        "JSON for Modern C++",
    ),
    (
        "catch2",
        "https://github.com/catchorg/Catch2.git",
        "C++ test framework",
    ),
];

/// Framework subcommand operations
#[derive(Clone, Debug)]
pub enum FrameworkOp {
    /// List all available frameworks
    List,
    /// Interactively select a framework
    Select,
    /// Add a specific framework by name
    Add { name: String },
    /// Remove framework from current project
    Remove { name: String },
    /// Show framework info
    Info { name: String },
}

/// Get framework info by name
pub fn get_framework(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    FRAMEWORKS
        .iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
        .copied()
}

/// Handle the `cx framework` command
pub fn handle_framework_command(op: &Option<FrameworkOp>) -> Result<()> {
    match op {
        Some(FrameworkOp::List) | None => {
            println!("\n{}", "📦 Available Frameworks:".bold());
            let mut table = ui::Table::new(&["Name", "Description"]);

            for (name, _, desc) in FRAMEWORKS {
                table.add_row(vec![
                    name.cyan().bold().to_string(),
                    desc.dimmed().to_string(),
                ]);
            }
            table.print();

            println!("\n{}", "Usage:".bold());
            println!(
                "  {} - Add to current project",
                "cx framework add <name>".cyan()
            );
            println!("  {} - Interactive selection", "cx framework select".cyan());
            println!(
                "  {} - Add to cx.toml: {}",
                "Or".dimmed(),
                "framework = \"daxe\"".yellow()
            );
        }

        Some(FrameworkOp::Select) => {
            let options: Vec<String> = FRAMEWORKS
                .iter()
                .map(|(name, _, desc)| format!("{} - {}", name, desc))
                .collect();

            let selection = Select::new("Select a framework to add:", options).prompt()?;

            // Parse selected name
            let name = selection.split(" - ").next().unwrap_or("");
            if let Some((fw_name, _, desc)) = get_framework(name) {
                add_framework_to_toml(fw_name)?;
                println!(
                    "\n{} Added framework: {} ({})",
                    "✓".green(),
                    fw_name.cyan().bold(),
                    desc.dimmed()
                );
            }
        }

        Some(FrameworkOp::Add { name }) => {
            if let Some((fw_name, _, desc)) = get_framework(name) {
                add_framework_to_toml(fw_name)?;
                println!(
                    "{} Added framework: {} ({})",
                    "✓".green(),
                    fw_name.cyan().bold(),
                    desc.dimmed()
                );
            } else {
                println!("{} Unknown framework: {}", "✗".red(), name.yellow());
                println!(
                    "  Use {} to see available frameworks",
                    "cx framework list".cyan()
                );
            }
        }

        Some(FrameworkOp::Remove { name }) => {
            remove_framework_from_toml(name)?;
            println!("{} Removed framework: {}", "✓".green(), name.cyan());
        }

        Some(FrameworkOp::Info { name }) => {
            if let Some((fw_name, url, desc)) = get_framework(name) {
                println!("\n{}", "📦 Framework Info:".bold());
                println!("  Name: {}", fw_name.cyan().bold());
                println!("  Description: {}", desc);
                println!("  URL: {}", url.dimmed());
            } else {
                println!("{} Unknown framework: {}", "✗".red(), name.yellow());
            }
        }
    }

    Ok(())
}

/// Add framework to cx.toml
fn add_framework_to_toml(name: &str) -> Result<()> {
    use std::path::Path;

    let toml_path = Path::new("cx.toml");

    if !toml_path.exists() {
        // Create minimal cx.toml with framework
        let content = format!(
            r#"[package]
name = "untitled"
version = "0.1.0"
edition = "c++20"

[build]
framework = "{}"
"#,
            name
        );
        std::fs::write(toml_path, content)?;
        println!(
            "  {} Created cx.toml with framework = \"{}\"",
            "✓".green(),
            name.cyan()
        );
        return Ok(());
    }

    // Read and update existing cx.toml
    let content = std::fs::read_to_string(toml_path)?;

    let new_content = if content.contains("[build]") {
        if content.contains("framework =") {
            // Replace existing framework
            let mut result = String::new();
            for line in content.lines() {
                if line.trim().starts_with("framework =") {
                    result.push_str(&format!("framework = \"{}\"", name));
                } else {
                    result.push_str(line);
                }
                result.push('\n');
            }
            result
        } else {
            // Add framework to existing [build] section
            content.replace("[build]", &format!("[build]\nframework = \"{}\"", name))
        }
    } else {
        // Add new [build] section
        format!(
            "{}\n[build]\nframework = \"{}\"\n",
            content.trim_end(),
            name
        )
    };

    std::fs::write(toml_path, new_content)?;
    Ok(())
}

/// Remove framework from cx.toml
fn remove_framework_from_toml(_name: &str) -> Result<()> {
    use std::path::Path;

    let toml_path = Path::new("cx.toml");
    if !toml_path.exists() {
        return Err(anyhow::anyhow!("cx.toml not found"));
    }

    let content = std::fs::read_to_string(toml_path)?;
    let mut result = String::new();

    for line in content.lines() {
        if !line.trim().starts_with("framework =") {
            result.push_str(line);
            result.push('\n');
        }
    }

    std::fs::write(toml_path, result)?;
    Ok(())
}
