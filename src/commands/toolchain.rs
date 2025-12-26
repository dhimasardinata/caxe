//! Toolchain command handler
//!
//! Handles `cx toolchain` subcommands for managing C++ toolchains.

use anyhow::Result;
use colored::*;
use inquire::Select;
use std::path::{Path, PathBuf};

use crate::build;
use crate::toolchain;
use crate::ui;

/// Toolchain subcommand operations
#[derive(Clone, Debug)]
pub enum ToolchainOp {
    /// List all available toolchains
    List,
    /// Interactively select a toolchain
    Select,
    /// Clear cached toolchain selection
    Clear,
    /// Install a portable toolchain
    Install { name: Option<String> },
    /// Update/refresh toolchain cache
    Update,
}

/// Handle the `cx toolchain` command
pub fn handle_toolchain_command(op: &Option<ToolchainOp>) -> Result<()> {
    #[cfg(windows)]
    {
        use toolchain::windows::discover_all_toolchains;

        match op {
            Some(ToolchainOp::List) => {
                let toolchains = discover_all_toolchains();
                if toolchains.is_empty() {
                    println!("{} No toolchains found.", "x".red());
                } else {
                    // Try to detect active toolchain to highlight it
                    let config = build::load_config().ok();
                    let preferred_type = config
                        .as_ref()
                        .and_then(|c| c.build.as_ref())
                        .and_then(|b| b.compiler.as_ref())
                        .map(|s| match s.as_str() {
                            "clang-cl" => toolchain::CompilerType::ClangCL,
                            "clang" => toolchain::CompilerType::Clang,
                            "g++" | "gcc" => toolchain::CompilerType::GCC,
                            _ => toolchain::CompilerType::MSVC,
                        });

                    let active = toolchain::get_or_detect_toolchain(preferred_type, false).ok();

                    println!("{} Available Toolchains:", "Available Toolchains:".bold());
                    let mut table = ui::Table::new(&["Id", "Name", "Version", "Source"]);

                    for (i, tc) in toolchains.iter().enumerate() {
                        let is_in_use = if let Some(a) = &active {
                            tc.path == a.cc_path || tc.path == a.cxx_path
                        } else {
                            false
                        };

                        let short_ver = if tc.version.len() > 40 {
                            format!("{}...", &tc.version[..40])
                        } else {
                            tc.version.clone()
                        };

                        let mut row = vec![
                            format!("{}", i + 1),
                            tc.display_name.clone(),
                            short_ver,
                            tc.source.to_string(),
                        ];

                        if is_in_use {
                            row = row
                                .into_iter()
                                .map(|s| s.green().bold().to_string())
                                .collect();
                        } else {
                            row[0] = row[0].dimmed().to_string();
                            row[1] = row[1].cyan().to_string();
                            row[2] = row[2].dimmed().to_string();
                            row[3] = row[3].yellow().to_string();
                        }

                        table.add_row(row);
                    }
                    table.print();
                }
            }

            None | Some(ToolchainOp::Select) => {
                // Interactive selection (default behavior)
                let toolchains = discover_all_toolchains();
                if toolchains.is_empty() {
                    println!("{} No toolchains found!", "x".red());
                    println!("  Install Visual Studio Build Tools or LLVM to get started.");
                    return Ok(());
                }

                // Format options for display
                let options: Vec<String> = toolchains.iter().map(|tc| tc.to_string()).collect();

                let selection = Select::new("Select a toolchain:", options).prompt()?;

                // Find the selected toolchain
                let selected = toolchains.iter().find(|tc| tc.to_string() == selection);

                if let Some(tc) = selected {
                    // Cache the selection
                    let cache_path = dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".cx")
                        .join("toolchain-selection.toml");

                    let content = format!(
                        "# User-selected toolchain\ncompiler_type = {:?}\npath = {:?}\nversion = {:?}\nsource = {:?}\n",
                        format!("{:?}", tc.compiler_type),
                        tc.path.display(),
                        tc.version,
                        tc.source
                    );

                    if let Some(parent) = cache_path.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            println!("{} Failed to create cache dir: {}", "x".red(), e);
                        } else if let Err(e) = std::fs::write(&cache_path, content) {
                            println!("{} Failed to save selection: {}", "x".red(), e);
                        } else {
                            println!();
                            println!(
                                "{} Selected: {} ({})",
                                "✓".green(),
                                tc.display_name.cyan(),
                                tc.source.yellow()
                            );
                            println!("  Saved to: {}", cache_path.display().to_string().dimmed());
                        }
                    }

                    // Also update cx.toml if we're in a project
                    if Path::new("cx.toml").exists() {
                        let compiler_str = match tc.compiler_type {
                            toolchain::CompilerType::MSVC => "msvc",
                            toolchain::CompilerType::ClangCL => "clang-cl",
                            toolchain::CompilerType::Clang => "clang",
                            toolchain::CompilerType::GCC => "g++",
                        };

                        // Read current cx.toml
                        if let Ok(toml_content) = std::fs::read_to_string("cx.toml") {
                            let new_content = if toml_content.contains("[build]") {
                                // Update existing [build] section
                                if toml_content.contains("compiler =") {
                                    // Replace existing compiler line
                                    let mut result = String::new();
                                    for line in toml_content.lines() {
                                        if line.trim().starts_with("compiler =") {
                                            result.push_str(&format!(
                                                "compiler = \"{}\"",
                                                compiler_str
                                            ));
                                        } else {
                                            result.push_str(line);
                                        }
                                        result.push('\n');
                                    }
                                    result
                                } else {
                                    // Add compiler to existing [build] section
                                    toml_content.replace(
                                        "[build]",
                                        &format!("[build]\ncompiler = \"{}\"", compiler_str),
                                    )
                                }
                            } else {
                                // Add new [build] section
                                format!(
                                    "{}\n[build]\ncompiler = \"{}\"\n",
                                    toml_content.trim_end(),
                                    compiler_str
                                )
                            };

                            if let Err(e) = std::fs::write("cx.toml", new_content) {
                                println!("{} Failed to update cx.toml: {}", "x".red(), e);
                            } else {
                                println!(
                                    "  {} Updated cx.toml with compiler = \"{}\"",
                                    "✓".green(),
                                    compiler_str.cyan()
                                );
                            }
                        }
                    }
                }
            }

            Some(ToolchainOp::Clear) => {
                // Clear cached selection
                let cache_path = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".cx")
                    .join("toolchain-selection.toml");

                if cache_path.exists() {
                    if let Err(e) = std::fs::remove_file(&cache_path) {
                        println!("{} Failed to clear selection: {}", "x".red(), e);
                    } else {
                        println!("{} Cleared toolchain selection", "✓".green());
                    }
                } else {
                    println!("{} No selection cached.", "!".yellow());
                }
            }

            Some(ToolchainOp::Install { name }) => {
                toolchain::install::install_toolchain(name.clone())?;
            }

            Some(ToolchainOp::Update) => {
                toolchain::install::update_toolchains()?;
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = op; // Suppress unused warning
        println!(
            "{} Toolchain management is currently Windows-only.",
            "!".yellow()
        );
    }

    Ok(())
}
