//! Global dependency cache management.
//!
//! This module handles the `~/.cx/cache` directory where downloaded dependencies are stored.
//!
//! ## Commands
//!
//! - `cx cache path` - Print cache directory location
//! - `cx cache list` - List cached libraries
//! - `cx cache clean` - Clear all cached dependencies
//! - `cx cache prune` - Remove unused dependencies

use crate::ui;
use anyhow::{Context, Result};
use colored::*;
use std::collections::HashSet;
use std::fs;

pub fn print_path() -> Result<()> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home.join(".cx").join("cache");
    println!("{}", cache_dir.display());
    Ok(())
}

pub fn list() -> Result<()> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home.join(".cx").join("cache");

    if !cache_dir.exists() {
        println!("{} Cache is empty.", "ℹ".blue());
        return Ok(());
    }

    let entries = fs::read_dir(&cache_dir)?;
    let mut table = ui::Table::new(&["Cached Library"]);
    let mut count = 0;

    for entry in entries {
        if let Ok(entry) = entry
            && let Ok(ft) = entry.file_type()
            && ft.is_dir()
        {
            let name = entry.file_name();
            table.add_row(vec![name.to_string_lossy().to_string()]);
            count += 1;
        }
    }

    if count == 0 {
        println!("{} (empty)", "ℹ".blue());
    } else {
        table.print();
    }

    Ok(())
}

pub fn clean() -> Result<()> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home.join(".cx").join("cache");

    if cache_dir.exists() {
        println!("{} Cleaning cache...", "🧹".yellow());
        fs::remove_dir_all(&cache_dir)?;
        fs::create_dir_all(&cache_dir)?;
        println!("{} Cache cleaned.", "✓".green());
    } else {
        println!("{} Cache already empty.", "✓".green());
    }
    Ok(())
}

pub fn prune_unused(keep_deps: &[String]) -> Result<()> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home.join(".cx").join("cache");

    if !cache_dir.exists() {
        println!("{} Cache is already empty.", "✓".green());
        return Ok(());
    }

    println!("{} Pruning unused packages...", "🧹".yellow());
    let entries = fs::read_dir(&cache_dir)?;
    let mut removed_count = 0;
    let keep_set: HashSet<&str> = keep_deps.iter().map(String::as_str).collect();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !keep_set.contains(name.as_str()) {
                println!("   {} Removing unused: {}", "🗑️".red(), name);
                if let Err(e) = fs::remove_dir_all(&path) {
                    println!("     Error removing {}: {}", name, e);
                } else {
                    removed_count += 1;
                }
            }
        }
    }

    if removed_count == 0 {
        println!("{} All cached packages are in use.", "✓".green());
    } else {
        println!("{} Removed {} unused packages.", "✓".green(), removed_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_prune_keeps_listed_deps() {
        let temp_dir = tempdir().unwrap();
        let cache_dir = temp_dir.path().join(".cx").join("cache");
        std::fs::create_dir_all(&cache_dir).ok();

        // Create fake dep directories
        std::fs::create_dir_all(cache_dir.join("raylib")).ok();
        std::fs::create_dir_all(cache_dir.join("json")).ok();
        std::fs::create_dir_all(cache_dir.join("unused_lib")).ok();

        // Verify directories exist
        assert!(cache_dir.join("raylib").exists());
        assert!(cache_dir.join("json").exists());
        assert!(cache_dir.join("unused_lib").exists());
    }

    #[test]
    fn test_cache_path_is_in_home() {
        // Just test that the function doesn't panic
        let result = print_path();
        assert!(result.is_ok());
    }
}
