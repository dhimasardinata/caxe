use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;

pub fn clean() -> Result<()> {
    let mut cleaned = false;

    if Path::new("build").exists() {
        fs::remove_dir_all("build").context("Failed to remove build directory")?;
        cleaned = true;
    }

    if Path::new("compile_commands.json").exists() {
        fs::remove_file("compile_commands.json").context("Failed to remove compile commands")?;
        cleaned = true;
    }

    if cleaned {
        println!(
            "{} Project cleaned (build/ & metadata removed)",
            "✓".green()
        );
    } else {
        println!("{} Nothing to clean", "!".yellow());
    }
    Ok(())
}
