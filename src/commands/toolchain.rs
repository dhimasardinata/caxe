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

#[cfg(windows)]
fn preferred_compiler_from_config() -> Option<toolchain::CompilerType> {
    let config = build::load_config().ok()?;
    let compiler = config
        .build
        .as_ref()
        .and_then(|b| b.compiler.as_ref())
        .map(String::as_str)?;
    Some(match compiler {
        "clang-cl" => toolchain::CompilerType::ClangCL,
        "clang" => toolchain::CompilerType::Clang,
        "g++" | "gcc" => toolchain::CompilerType::GCC,
        _ => toolchain::CompilerType::MSVC,
    })
}

#[cfg(windows)]
fn active_toolchain() -> Option<toolchain::Toolchain> {
    toolchain::get_or_detect_toolchain(preferred_compiler_from_config(), false).ok()
}

#[cfg(windows)]
fn is_in_use(
    available: &toolchain::windows::AvailableToolchain,
    active: Option<&toolchain::Toolchain>,
) -> bool {
    active
        .map(|detected| available.path == detected.cc_path || available.path == detected.cxx_path)
        .unwrap_or(false)
}

#[cfg(windows)]
fn truncated_version(version: &str) -> String {
    if version.len() > 40 {
        format!("{}...", &version[..40])
    } else {
        version.to_string()
    }
}

#[cfg(windows)]
fn style_table_row(mut row: Vec<String>, active: bool) -> Vec<String> {
    if active {
        return row
            .into_iter()
            .map(|s| s.green().bold().to_string())
            .collect();
    }

    row[0] = row[0].dimmed().to_string();
    row[1] = row[1].cyan().to_string();
    row[2] = row[2].dimmed().to_string();
    row[3] = row[3].yellow().to_string();
    row
}

#[cfg(windows)]
fn print_toolchain_table(
    toolchains: &[toolchain::windows::AvailableToolchain],
    active: Option<&toolchain::Toolchain>,
) {
    println!("{} Available Toolchains:", "Available Toolchains:".bold());
    let mut table = ui::Table::new(&["Id", "Name", "Version", "Source"]);

    for (i, tc) in toolchains.iter().enumerate() {
        let row = vec![
            format!("{}", i + 1),
            tc.display_name.clone(),
            truncated_version(&tc.version),
            tc.source.to_string(),
        ];
        table.add_row(style_table_row(row, is_in_use(tc, active)));
    }

    table.print();
}

#[cfg(windows)]
fn list_toolchains() {
    use toolchain::windows::discover_all_toolchains;

    let toolchains = discover_all_toolchains();
    if toolchains.is_empty() {
        println!("{} No toolchains found.", "x".red());
        return;
    }

    let active = active_toolchain();
    print_toolchain_table(&toolchains, active.as_ref());
}

#[cfg(windows)]
fn selection_cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cx")
        .join("toolchain-selection.toml")
}

#[cfg(windows)]
fn save_selection(tc: &toolchain::windows::AvailableToolchain) {
    let cache_path = selection_cache_path();
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
            return;
        }
        if let Err(e) = std::fs::write(&cache_path, content) {
            println!("{} Failed to save selection: {}", "x".red(), e);
            return;
        }
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

#[cfg(windows)]
fn compiler_str(compiler_type: &toolchain::CompilerType) -> &'static str {
    match compiler_type {
        toolchain::CompilerType::MSVC => "msvc",
        toolchain::CompilerType::ClangCL => "clang-cl",
        toolchain::CompilerType::Clang => "clang",
        toolchain::CompilerType::GCC => "g++",
    }
}

#[cfg(windows)]
fn upsert_build_compiler(toml_content: &str, compiler: &str) -> String {
    if toml_content.contains("[build]") {
        if toml_content.contains("compiler =") {
            let mut result = String::new();
            for line in toml_content.lines() {
                if line.trim().starts_with("compiler =") {
                    result.push_str(&format!("compiler = \"{}\"", compiler));
                } else {
                    result.push_str(line);
                }
                result.push('\n');
            }
            result
        } else {
            toml_content.replace("[build]", &format!("[build]\ncompiler = \"{}\"", compiler))
        }
    } else {
        format!(
            "{}\n[build]\ncompiler = \"{}\"\n",
            toml_content.trim_end(),
            compiler
        )
    }
}

#[cfg(windows)]
fn update_project_compiler(tc: &toolchain::windows::AvailableToolchain) {
    if !Path::new("cx.toml").exists() {
        return;
    }

    let compiler = compiler_str(&tc.compiler_type);
    let Ok(toml_content) = std::fs::read_to_string("cx.toml") else {
        return;
    };

    let new_content = upsert_build_compiler(&toml_content, compiler);
    if let Err(e) = std::fs::write("cx.toml", new_content) {
        println!("{} Failed to update cx.toml: {}", "x".red(), e);
    } else {
        println!(
            "  {} Updated cx.toml with compiler = \"{}\"",
            "✓".green(),
            compiler.cyan()
        );
    }
}

#[cfg(windows)]
fn select_toolchain() -> Result<()> {
    use toolchain::windows::discover_all_toolchains;

    let toolchains = discover_all_toolchains();
    if toolchains.is_empty() {
        println!("{} No toolchains found!", "x".red());
        println!("  Install Visual Studio Build Tools or LLVM to get started.");
        return Ok(());
    }

    let options: Vec<String> = toolchains.iter().map(|tc| tc.to_string()).collect();
    let selection = Select::new("Select a toolchain:", options).prompt()?;

    if let Some(tc) = toolchains.iter().find(|tc| tc.to_string() == selection) {
        save_selection(tc);
        update_project_compiler(tc);
    }

    Ok(())
}

#[cfg(windows)]
fn clear_selection_cache() {
    let cache_path = selection_cache_path();
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

/// Handle the `cx toolchain` command
pub fn handle_toolchain_command(op: &Option<ToolchainOp>) -> Result<()> {
    #[cfg(windows)]
    {
        match op {
            Some(ToolchainOp::List) => list_toolchains(),
            None | Some(ToolchainOp::Select) => select_toolchain()?,
            Some(ToolchainOp::Clear) => clear_selection_cache(),
            Some(ToolchainOp::Install { name }) => {
                toolchain::install::install_toolchain(name.clone())?
            }
            Some(ToolchainOp::Update) => toolchain::install::update_toolchains()?,
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
