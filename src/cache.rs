use crate::ui;
use anyhow::{Context, Result};
use colored::*;
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
        if let Ok(entry) = entry {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    let name = entry.file_name();
                    table.add_row(vec![name.to_string_lossy().to_string()]);
                    count += 1;
                }
            }
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

    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !keep_deps.contains(&name) {
                    println!("   {} Removing unused: {}", "🗑️".red(), name);
                    if let Err(e) = fs::remove_dir_all(&path) {
                        println!("     Error removing {}: {}", name, e);
                    } else {
                        removed_count += 1;
                    }
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
