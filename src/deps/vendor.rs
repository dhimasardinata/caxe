//! Dependency vendoring for offline builds.
//!
//! This module provides the `cx vendor` command which copies cached dependencies
//! into a local `vendor/` directory for reproducible, offline builds.
//!
//! ## Usage
//!
//! ```bash
//! cx vendor  # Copies ~/.cx/cache/* to ./vendor/
//! ```

use crate::build::load_config;
use crate::config::Dependency;
use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;

pub fn vendor_dependencies() -> Result<()> {
    // 1. Load Config
    let config = load_config()?;
    let deps = match config.dependencies {
        Some(d) => d,
        None => {
            println!("{} No dependencies found in cx.toml", "!".yellow());
            return Ok(());
        }
    };

    if deps.is_empty() {
        println!("{} No dependencies to vendor.", "!".yellow());
        return Ok(());
    }

    // 2. Prepare vendor directory
    let vendor_dir = Path::new("vendor");
    if !vendor_dir.exists() {
        fs::create_dir(vendor_dir)?;
    }

    // 3. Resolve Cache Path
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");

    println!(
        "{} Vendoring {} dependencies to ./vendor...",
        "📦".blue(),
        deps.len()
    );

    for (name, dep) in deps {
        // Skip pkg-config deps
        if let Dependency::Complex { pkg: Some(_), .. } = dep {
            continue;
        }

        let source_path = cache_dir.join(&name);
        let dest_path = vendor_dir.join(&name);

        if !source_path.exists() {
            println!(
                "{} Source not found in cache: {}. Run 'cx update' first.",
                "x".red(),
                name
            );
            continue;
        }

        if dest_path.exists() {
            println!("   {} Updating {}", "⚡".yellow(), name);
            fs::remove_dir_all(&dest_path)?;
        } else {
            println!("   {} Copying {}", "+".green(), name);
        }

        copy_dir_all(&source_path, &dest_path)?;
    }

    println!("{} Vendor complete.", "✓".green());
    Ok(())
}

// Simple recursive copy
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
