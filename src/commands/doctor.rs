//! Doctor command handler
//!
//! Handles `cx doctor`, `cx lock`, and `cx sync` commands.

use anyhow::Result;
use colored::*;

use crate::build;
use crate::deps;
use crate::lock;
use crate::toolchain;

/// Run the `cx doctor` command to diagnose system issues
pub fn run_doctor() -> Result<()> {
    println!("{} Running System Doctor...", "🚑".red());
    println!("-------------------------------");

    print!("Checking OS... ");
    println!(
        "{} ({})",
        std::env::consts::OS.green(),
        std::env::consts::ARCH.cyan()
    );

    #[cfg(windows)]
    {
        print!("Checking MSVC... ");
        let toolchains = toolchain::windows::discover_all_toolchains();
        if !toolchains.is_empty() {
            println!("{}", "Found".green());
            for tc in toolchains {
                println!("  - {} ({})", tc.display_name, tc.version);
            }
        } else {
            println!("{}", "Not Found (Install Visual Studio Build Tools)".red());
        }
    }

    print!("Checking Git... ");
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok()
    {
        println!("{}", "Found".green());
    } else {
        println!("{}", "Not Found (Install Git)".red());
    }

    // Check CMake
    print!("Checking CMake... ");
    if std::process::Command::new("cmake")
        .arg("--version")
        .output()
        .is_ok()
    {
        println!("{}", "Found".green());
    } else {
        println!("{}", "Not Found (Optional)".yellow());
    }

    Ok(())
}

/// Handle the `cx lock` command for managing lockfiles
pub fn handle_lock(update: bool, check: bool) {
    if check {
        println!("{} Verifying lockfile...", "🔒".blue());
        match lock::LockFile::load() {
            Ok(lockfile) => match build::load_config() {
                Ok(config) => {
                    let mut success = true;
                    if let Some(deps) = config.dependencies {
                        for (name, _) in deps {
                            if lockfile.get(&name).is_none() {
                                println!(
                                    "{} Dependency '{}' missing from cx.lock",
                                    "x".red(),
                                    name
                                );
                                success = false;
                            }
                        }
                    }
                    if success {
                        println!("{} Lockfile is in sync.", "✓".green());
                    } else {
                        println!(
                            "{} Lockfile out of sync. Run 'cx lock --update'.",
                            "x".red()
                        );
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error loading config: {}", e);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("Error loading lockfile: {}", e);
                std::process::exit(1);
            }
        }
    } else if update {
        println!("{} Updating lockfile...", "🔄".blue());
        if let Err(e) = deps::update_dependencies() {
            eprintln!("Error updating dependencies: {}", e);
            std::process::exit(1);
        }
    } else {
        println!("Use --check to verify or --update to update/regenerate.");
    }
}

/// Handle the `cx sync` command for synchronizing dependencies
pub fn handle_sync() {
    println!(
        "{} Synchronizing dependencies with lockfile...",
        "📦".blue()
    );
    // 1. Load Config to check if we even have deps
    match build::load_config() {
        Ok(config) => {
            if let Some(deps) = config.dependencies {
                // 2. Fetch/Sync
                // fetch_dependencies handles reading cx.lock and checking out specific revisions
                match deps::fetch_dependencies(&deps) {
                    Ok(_) => println!("{} Dependencies synchronized.", "✓".green()),
                    Err(e) => {
                        eprintln!("Error synchronizing: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("No dependencies found in cx.toml.");
            }
        }
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    }
}
