//! Framework command handler
//!
//! Handles `cx framework` subcommands and keeps behavior aligned with
//! actual build integration support.

use anyhow::{Result, anyhow};
use colored::*;
use inquire::Select;
use std::path::Path;

use crate::ui;

/// Support level for framework entries shown by `cx framework`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameworkSupport {
    /// Fully integrated through `[build].framework` behavior.
    Integrated,
    /// Alias entry that should be installed via `cx add <name>`.
    DependencyAlias,
}

impl FrameworkSupport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Integrated => "integrated",
            Self::DependencyAlias => "dependency-alias",
        }
    }

    pub fn recommended_command(self, name: &str) -> String {
        match self {
            Self::Integrated => format!("cx framework add {name}"),
            Self::DependencyAlias => format!("cx add {name}"),
        }
    }
}

/// Built-in framework metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameworkSpec {
    pub name: &'static str,
    pub url: &'static str,
    pub description: &'static str,
    pub support: FrameworkSupport,
}

/// Built-in frameworks with support metadata and source URLs.
pub const FRAMEWORKS: &[FrameworkSpec] = &[
    FrameworkSpec {
        name: "daxe",
        url: "https://github.com/dhimasardinata/daxe.git",
        description: "D.A's Axe - Cut through C++ verbosity",
        support: FrameworkSupport::Integrated,
    },
    FrameworkSpec {
        name: "fmt",
        url: "https://github.com/fmtlib/fmt.git",
        description: "Modern formatting library",
        support: FrameworkSupport::DependencyAlias,
    },
    FrameworkSpec {
        name: "spdlog",
        url: "https://github.com/gabime/spdlog.git",
        description: "Fast C++ logging library",
        support: FrameworkSupport::DependencyAlias,
    },
    FrameworkSpec {
        name: "json",
        url: "https://github.com/nlohmann/json.git",
        description: "JSON for Modern C++",
        support: FrameworkSupport::DependencyAlias,
    },
    FrameworkSpec {
        name: "catch2",
        url: "https://github.com/catchorg/Catch2.git",
        description: "C++ test framework",
        support: FrameworkSupport::DependencyAlias,
    },
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
pub fn get_framework(name: &str) -> Option<&'static FrameworkSpec> {
    FRAMEWORKS
        .iter()
        .find(|spec| spec.name.eq_ignore_ascii_case(name))
}

/// Handle the `cx framework` command
pub fn handle_framework_command(op: &Option<FrameworkOp>) -> Result<()> {
    match op {
        Some(FrameworkOp::List) | None => {
            println!("\n{}", "📦 Available Frameworks:".bold());
            let mut table = ui::Table::new(&["Name", "Status", "Description", "Recommended"]);

            for spec in FRAMEWORKS {
                table.add_row(vec![
                    spec.name.cyan().bold().to_string(),
                    support_label(spec.support),
                    spec.description.dimmed().to_string(),
                    spec.support
                        .recommended_command(spec.name)
                        .green()
                        .to_string(),
                ]);
            }
            table.print();

            println!("\n{}", "Usage:".bold());
            println!(
                "  {} - Add integrated frameworks",
                "cx framework add daxe".cyan()
            );
            println!("  {} - Interactive selection", "cx framework select".cyan());
            println!(
                "  {} - Add dependency-alias entries",
                "cx add <name>".cyan()
            );
            println!(
                "  {} - Configure directly in {} (integrated only): {}",
                "Or".dimmed(),
                "cx.toml".yellow(),
                "framework = \"daxe\"".yellow()
            );
        }

        Some(FrameworkOp::Select) => {
            let options: Vec<String> = FRAMEWORKS
                .iter()
                .map(|spec| {
                    format!(
                        "{} [{}] - {}",
                        spec.name,
                        spec.support.as_str(),
                        spec.description
                    )
                })
                .collect();

            let selection = Select::new("Select a framework to add:", options).prompt()?;

            let Some(name) = selection.split(' ').next() else {
                return Err(anyhow!("Unable to parse framework selection"));
            };
            let Some(spec) = get_framework(name) else {
                return Err(anyhow!("Unknown framework: {name}"));
            };
            add_framework(spec)?;
        }

        Some(FrameworkOp::Add { name }) => {
            let Some(spec) = get_framework(name) else {
                eprintln!("{} Unknown framework: {}", "✗".red(), name.yellow());
                eprintln!(
                    "  Use {} to see available frameworks",
                    "cx framework list".cyan()
                );
                return Err(anyhow!("Unknown framework: {name}"));
            };
            add_framework(spec)?;
        }

        Some(FrameworkOp::Remove { name }) => {
            remove_framework_from_toml(name)?;
            println!("{} Removed framework: {}", "✓".green(), name.cyan());
        }

        Some(FrameworkOp::Info { name }) => {
            let Some(spec) = get_framework(name) else {
                eprintln!("{} Unknown framework: {}", "✗".red(), name.yellow());
                return Err(anyhow!("Unknown framework: {name}"));
            };

            println!("\n{}", "📦 Framework Info:".bold());
            println!("  Name: {}", spec.name.cyan().bold());
            println!("  Description: {}", spec.description);
            println!("  Status: {}", support_label(spec.support));
            println!(
                "  Recommended: {}",
                spec.support.recommended_command(spec.name).green()
            );
            println!("  URL: {}", spec.url.dimmed());
        }
    }

    Ok(())
}

fn add_framework(spec: &FrameworkSpec) -> Result<()> {
    match spec.support {
        FrameworkSupport::Integrated => {
            add_framework_to_toml(spec.name)?;
            println!(
                "{} Added framework: {} ({})",
                "✓".green(),
                spec.name.cyan().bold(),
                spec.description.dimmed()
            );
            Ok(())
        }
        FrameworkSupport::DependencyAlias => {
            let cmd = spec.support.recommended_command(spec.name);
            eprintln!(
                "{} Framework '{}' is a {} entry and is not integrated via {}.",
                "✗".red(),
                spec.name.cyan().bold(),
                spec.support.as_str().yellow(),
                "[build].framework".yellow()
            );
            eprintln!("  Use {} instead.", cmd.cyan());
            Err(anyhow!(
                "Framework '{}' must be added with `{cmd}`",
                spec.name
            ))
        }
    }
}

fn support_label(support: FrameworkSupport) -> String {
    match support {
        FrameworkSupport::Integrated => support.as_str().green().bold().to_string(),
        FrameworkSupport::DependencyAlias => support.as_str().yellow().to_string(),
    }
}

/// Add framework to cx.toml
fn add_framework_to_toml(name: &str) -> Result<()> {
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
    let new_content = set_build_framework_in_content(&content, name);

    std::fs::write(toml_path, new_content)?;
    Ok(())
}

/// Remove framework from cx.toml
fn remove_framework_from_toml(_name: &str) -> Result<()> {
    let toml_path = Path::new("cx.toml");
    if !toml_path.exists() {
        return Err(anyhow::anyhow!("cx.toml not found"));
    }

    let content = std::fs::read_to_string(toml_path)?;
    let new_content = remove_build_framework_from_content(&content);
    std::fs::write(toml_path, new_content)?;
    Ok(())
}

fn set_build_framework_in_content(content: &str, framework: &str) -> String {
    let framework_line = format!("framework = \"{framework}\"");
    let mut output = String::new();
    let mut in_build_section = false;
    let mut seen_build_section = false;
    let mut wrote_framework_in_build = false;

    for line in content.lines() {
        if let Some(section_name) = parse_section_name(line) {
            if in_build_section && !wrote_framework_in_build {
                output.push_str(&framework_line);
                output.push('\n');
                wrote_framework_in_build = true;
            }

            in_build_section = section_name.eq_ignore_ascii_case("build");
            if in_build_section {
                seen_build_section = true;
                wrote_framework_in_build = false;
            }

            output.push_str(line);
            output.push('\n');
            continue;
        }

        if in_build_section && is_framework_assignment(line) {
            output.push_str(&framework_line);
            output.push('\n');
            wrote_framework_in_build = true;
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    if in_build_section && !wrote_framework_in_build {
        output.push_str(&framework_line);
        output.push('\n');
    }

    if !seen_build_section {
        if !output.is_empty() && !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str("[build]\n");
        output.push_str(&framework_line);
        output.push('\n');
    }

    output
}

fn remove_build_framework_from_content(content: &str) -> String {
    let mut output = String::new();
    let mut in_build_section = false;

    for line in content.lines() {
        if let Some(section_name) = parse_section_name(line) {
            in_build_section = section_name.eq_ignore_ascii_case("build");
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if in_build_section && is_framework_assignment(line) {
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    output
}

fn parse_section_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Some(trimmed.trim_start_matches('[').trim_end_matches(']').trim());
    }
    None
}

fn is_framework_assignment(line: &str) -> bool {
    assignment_key(line).is_some_and(|key| key == "framework")
}

fn assignment_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (left, _) = trimmed.split_once('=')?;
    let key = left.trim();
    if key.is_empty() {
        return None;
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_support_metadata_is_explicit() {
        let daxe = get_framework("daxe").expect("daxe should exist");
        assert_eq!(daxe.support, FrameworkSupport::Integrated);
        assert_eq!(
            daxe.support.recommended_command(daxe.name),
            "cx framework add daxe"
        );

        let fmt = get_framework("fmt").expect("fmt should exist");
        assert_eq!(fmt.support, FrameworkSupport::DependencyAlias);
        assert_eq!(fmt.support.recommended_command(fmt.name), "cx add fmt");
    }

    #[test]
    fn set_build_framework_replaces_only_build_section_key() {
        let input = r#"[package]
name = "demo"
framework = "package-keep"

[build]
sources = ["src/main.cpp"]
framework = "old-build"

[profile:esp32]
framework = "profile-keep"
"#;

        let output = set_build_framework_in_content(input, "daxe");

        assert!(output.contains("framework = \"package-keep\""));
        assert!(output.contains("framework = \"profile-keep\""));
        assert!(output.contains("[build]"));
        assert!(output.contains("framework = \"daxe\""));
        assert!(!output.contains("framework = \"old-build\""));

        let framework_keys = output
            .lines()
            .filter(|line| line.trim_start().starts_with("framework ="))
            .count();
        assert_eq!(framework_keys, 3);
    }

    #[test]
    fn set_build_framework_appends_build_section_when_missing() {
        let input = r#"[package]
name = "demo"
version = "0.1.0"
"#;

        let output = set_build_framework_in_content(input, "daxe");

        assert!(output.contains("[build]"));
        assert!(output.contains("framework = \"daxe\""));
        assert!(output.contains("name = \"demo\""));
    }

    #[test]
    fn remove_build_framework_removes_only_build_section_key() {
        let input = r#"[package]
name = "demo"
framework = "package-keep"

[build]
framework = "remove-me"
sources = ["src/main.cpp"]

[profile:esp32]
framework = "profile-keep"
"#;

        let output = remove_build_framework_from_content(input);

        assert!(output.contains("framework = \"package-keep\""));
        assert!(output.contains("framework = \"profile-keep\""));
        assert!(!output.contains("framework = \"remove-me\""));

        let framework_keys = output
            .lines()
            .filter(|line| line.trim_start().starts_with("framework ="))
            .count();
        assert_eq!(framework_keys, 2);
    }
}
