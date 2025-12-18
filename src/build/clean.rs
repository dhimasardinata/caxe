use anyhow::{Context, Result};
use colored::*;
use dirs;
use std::fs;
use std::path::Path;

pub fn clean(cache: bool, all: bool, unused: bool) -> Result<()> {
    let mut cleaned = false;

    // 1. Clean Build Directory (Default)
    if Path::new("build").exists() {
        fs::remove_dir_all("build").context("Failed to remove build directory")?;
        cleaned = true;
    }

    if Path::new("compile_commands.json").exists() {
        fs::remove_file("compile_commands.json").context("Failed to remove compile commands")?;
        cleaned = true;
    }

    if unused {
        if let Ok(config) = super::load_config() {
            let mut keep_deps = Vec::new();
            if let Some(deps) = config.dependencies {
                for (name, _) in deps {
                    keep_deps.push(name);
                }
            }
            crate::cache::prune_unused(&keep_deps)?;
            cleaned = true;
        } else {
            println!(
                "{} Could not load cx.toml to determine unused packages.",
                "!".yellow()
            );
        }
    }

    // 2. Clean Cache (Global)
    if cache {
        if let Some(home) = dirs::home_dir() {
            let cache_dir = home.join(".cx").join("cache");
            if cache_dir.exists() {
                println!(
                    "{} Cleaning global cache ({})",
                    "🗑️".red(),
                    cache_dir.display()
                );
                fs::remove_dir_all(&cache_dir).context("Failed to remove global cache")?;
                // Recreate it empty
                fs::create_dir_all(&cache_dir)?;
                cleaned = true;
            } else {
                println!("{} Global cache not found or already empty.", "!".yellow());
            }
        }
    }

    // 3. Clean All (Docs, etc.)
    if all {
        if Path::new("docs").exists() {
            fs::remove_dir_all("docs").context("Failed to remove docs")?;
            println!("{} Removed docs/", "🗑️".red());
            cleaned = true;
        }
        // Could add other artifacts here
    }

    if cleaned {
        println!("{} Clean complete.", "✓".green());
    } else {
        println!("{} Nothing to clean", "!".yellow());
    }
    Ok(())
}
