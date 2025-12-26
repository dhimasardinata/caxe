//! Arduino/IoT build and upload support.
//!
//! This module provides Arduino integration using `arduino-cli`.
//!
//! ## Commands
//!
//! - `cx build --arduino` - Compile Arduino sketch
//! - `cx upload -p <PORT>` - Upload to board

use anyhow::{Context, Result, bail};
use colored::*;
use std::process::Command;

use crate::config::CxConfig;

/// Build an Arduino project using arduino-cli
pub fn build_arduino(verbose: bool) -> Result<()> {
    println!("{} {}", "🔧".cyan(), "Building Arduino Project...".bold());
    println!();

    // Check for arduino-cli
    if !is_arduino_cli_available() {
        println!("{} arduino-cli not found!", "x".red());
        println!();
        println!("   Install arduino-cli:");
        #[cfg(windows)]
        println!("   {}", "winget install Arduino.Arduino-CLI".yellow());
        #[cfg(not(windows))]
        println!("   {}", "brew install arduino-cli".yellow());
        println!();
        println!(
            "   Or run {} for more options.",
            "cx toolchain install".cyan()
        );
        bail!("arduino-cli is required for Arduino builds");
    }

    // Load config
    let config = super::load_config().unwrap_or_default();

    // Find .ino file
    let sketch_path = find_sketch()?;
    println!("{} Sketch: {}", "→".dimmed(), sketch_path.display());

    // Get board from config or prompt
    let board = get_board(&config)?;
    println!("{} Board: {}", "→".dimmed(), board.cyan());

    // Build command
    let mut cmd = Command::new("arduino-cli");
    cmd.arg("compile");
    cmd.arg("--fqbn").arg(&board);

    // Add verbose flag
    if verbose {
        cmd.arg("-v");
    }

    // Add any extra flags from config
    if let Some(arduino_config) = &config.arduino
        && let Some(flags) = &arduino_config.flags
    {
        for flag in flags {
            cmd.arg(flag);
        }
    }

    // Add sketch path
    cmd.arg(&sketch_path);

    println!();
    if verbose {
        println!(
            "{} Running: arduino-cli compile --fqbn {} {}",
            "🏗️".blue(),
            board,
            sketch_path.display()
        );
    }

    // Execute
    let status = cmd.status().context("Failed to run arduino-cli")?;

    if status.success() {
        println!();
        println!("{} Build successful!", "✓".green());
        println!();
        println!(
            "   Upload with: {}",
            format!(
                "arduino-cli upload -p <PORT> --fqbn {} {}",
                board,
                sketch_path.display()
            )
            .yellow()
        );
    } else {
        bail!("Arduino build failed");
    }

    Ok(())
}

/// Check if arduino-cli is available
fn is_arduino_cli_available() -> bool {
    Command::new("arduino-cli")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Find the .ino sketch file
fn find_sketch() -> Result<std::path::PathBuf> {
    let current_dir = std::env::current_dir()?;

    // Look for .ino files in current directory
    for entry in std::fs::read_dir(&current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "ino") {
            return Ok(path);
        }
    }

    // Look in src/ directory
    let src_dir = current_dir.join("src");
    if src_dir.exists() {
        for entry in std::fs::read_dir(&src_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ino") {
                return Ok(path);
            }
        }
    }

    // Check if the directory name matches a sketch inside
    let dir_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let sketch_path = current_dir.join(format!("{}.ino", dir_name));
    if sketch_path.exists() {
        return Ok(sketch_path);
    }

    bail!("No .ino sketch file found. Create one or run from sketch directory.")
}

/// Get the board FQBN from config or use default
fn get_board(config: &CxConfig) -> Result<String> {
    // Check config first
    if let Some(arduino) = &config.arduino
        && let Some(board) = &arduino.board
    {
        return Ok(board.clone());
    }

    // Default to Arduino Uno
    println!(
        "{} No board specified in cx.toml, using default: arduino:avr:uno",
        "!".yellow()
    );
    println!("   Add [arduino] section to cx.toml:");
    println!("   {}", "[arduino]".dimmed());
    println!("   {}", "board = \"arduino:avr:uno\"".dimmed());
    println!();

    Ok("arduino:avr:uno".to_string())
}

/// Upload an Arduino sketch
pub fn upload_arduino(port: Option<String>, verbose: bool) -> Result<()> {
    println!("{} {}", "📤".cyan(), "Uploading Arduino Sketch...".bold());
    println!();

    if !is_arduino_cli_available() {
        bail!("arduino-cli not found");
    }

    let config = super::load_config().unwrap_or_default();
    let sketch_path = find_sketch()?;
    let board = get_board(&config)?;

    // Get port from arg, config, or auto-detect
    let port = port
        .or_else(|| config.arduino.as_ref().and_then(|a| a.port.clone()))
        .ok_or_else(|| anyhow::anyhow!("No port specified. Use --port or set in cx.toml"))?;

    println!("{} Port: {}", "→".dimmed(), port.cyan());

    let mut cmd = Command::new("arduino-cli");
    cmd.arg("upload");
    cmd.arg("-p").arg(&port);
    cmd.arg("--fqbn").arg(&board);

    if verbose {
        cmd.arg("-v");
    }

    cmd.arg(&sketch_path);

    let status = cmd.status().context("Failed to run arduino-cli upload")?;

    if status.success() {
        println!();
        println!("{} Upload successful!", "✓".green());
    } else {
        bail!("Arduino upload failed");
    }

    Ok(())
}
