//! Dependency management commands.
//!
//! This module provides functions for adding, removing, and updating dependencies.
//!
//! ## Commands
//!
//! - `cx add <lib>` - Add a dependency
//! - `cx remove <lib>` - Remove a dependency
//! - `cx update` - Update all dependencies to latest

use crate::config::Dependency;
use anyhow::{Context, Result};
use colored::*;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

// Needed imports for add/remove/update logic

pub fn add_dependency(
    lib_input: &str,
    tag: Option<String>,
    branch: Option<String>,
    rev: Option<String>,
) -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    // 1. Parse Input (Alias -> Short format -> URL)
    let (name, url) = if let Some(resolved_url) = crate::registry::resolve_alias(lib_input) {
        // Case A: Alias found (e.g. "raylib")
        (lib_input.to_string(), resolved_url)
    } else if lib_input.contains("http") || lib_input.contains("git@") {
        // Case B: Direct URL
        let name = lib_input
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .replace(".git", "");
        (name, lib_input.to_string())
    } else {
        // Case C: user/repo
        let parts: Vec<&str> = lib_input.split('/').collect();
        if parts.len() != 2 {
            println!(
                "{} Invalid format. Use 'alias', 'user/repo', or full URL.",
                "x".red()
            );
            return Ok(());
        }
        let name = parts[1].to_string();
        let url = format!("https://github.com/{}.git", lib_input);
        (name, url)
    };

    println!("{} Adding dependency: {}...", "📦".blue(), name.bold());

    // 2. Load Config
    let config_str = fs::read_to_string("cx.toml")?;
    let mut config: crate::config::CxConfig = toml::from_str(&config_str)?;

    if config.dependencies.is_none() {
        config.dependencies = Some(HashMap::new());
    }

    // 3. Construct Dependency Entry
    let dep_entry = if tag.is_none() && branch.is_none() && rev.is_none() {
        Dependency::Simple(url.clone())
    } else {
        Dependency::Complex {
            git: Some(url.clone()),
            pkg: None,
            branch,
            tag,
            rev,
            build: None,
            output: None,
        }
    };

    // 4. Insert & Save
    if let Some(deps) = &mut config.dependencies {
        if deps.contains_key(&name) {
            println!("! Dependency '{}' updated.", name);
        }
        deps.insert(name.clone(), dep_entry);
    }

    let new_toml = toml::to_string_pretty(&config)?;
    fs::write("cx.toml", new_toml)?;

    println!("{} Added {} to cx.toml", "✓".green(), name);

    // 5. Fetch immediately
    if let Some(deps) = &config.dependencies {
        let _ = super::fetch::fetch_dependencies(deps)?;
    }

    Ok(())
}

pub fn remove_dependency(name: &str) -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    let config_str = fs::read_to_string("cx.toml")?;
    let mut config: crate::config::CxConfig = toml::from_str(&config_str)?;

    let mut found = false;
    if let Some(deps) = &mut config.dependencies
        && deps.remove(name).is_some()
    {
        found = true;
    }

    if found {
        let new_toml = toml::to_string_pretty(&config)?;
        fs::write("cx.toml", new_toml)?;
        println!("{} Removed dependency: {}", "🗑️".red(), name.bold());
    } else {
        println!(
            "{} Dependency '{}' not found in cx.toml",
            "!".yellow(),
            name
        );
    }

    Ok(())
}

pub fn update_dependencies() -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    println!("{} Checking for updates...", "📦".blue());

    let config_str = fs::read_to_string("cx.toml")?;
    let config: crate::config::CxConfig = toml::from_str(&config_str)?;

    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");

    if let Some(deps) = config.dependencies {
        for (name, dep_data) in deps {
            let is_git = matches!(
                dep_data,
                crate::config::Dependency::Simple(_)
                    | crate::config::Dependency::Complex { git: Some(_), .. }
            );

            if is_git {
                let lib_path = cache_dir.join(&name);
                if lib_path.exists() {
                    print!("   Updating {} ... ", name);

                    if let Ok(repo) = git2::Repository::open(&lib_path) {
                        // Fetch origin
                        let mut remote = repo.find_remote("origin")?;
                        remote.fetch(&["HEAD"], None, None)?;

                        // Force checking out correct HEAD
                        // Note: For 'update', we typically want to pull latest.
                        // Use fetch + reset --hard to ensure we match upstream exactly, discarding local changes (it's a cache)
                        let command = "git fetch origin && git reset --hard origin/HEAD";
                        let status = if cfg!(target_os = "windows") {
                            Command::new("cmd")
                                .args(["/C", command])
                                .current_dir(&lib_path)
                                .output()
                        } else {
                            Command::new("sh")
                                .args(["-c", command])
                                .current_dir(&lib_path)
                                .output()
                        };

                        if let Ok(out) = status {
                            if out.status.success() {
                                println!("{}", "✓".green());
                            } else {
                                let err = String::from_utf8_lossy(&out.stderr);
                                println!("{} (git update failed: {})", "x".red(), err.trim());
                            }
                        } else {
                            println!("{}", "Error executing git".red());
                        }
                    } else {
                        println!("{}", "Not a valid git repo".yellow());
                    }
                }
            }
        }
    }

    println!("{} Dependencies updated.", "✓".green());
    Ok(())
}
