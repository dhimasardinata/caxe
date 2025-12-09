use crate::config::CxConfig;
use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;
use std::process::Command;

// --- Helper: Load Config Once ---
pub fn load_config() -> Result<CxConfig> {
    if !Path::new("cx.toml").exists() {
        return Err(anyhow::anyhow!("cx.toml not found"));
    }
    let config_str = fs::read_to_string("cx.toml")?;
    toml::from_str(&config_str).context("Failed to parse cx.toml")
}

// --- Helper: Get Compiler ---
pub fn get_compiler(config: &CxConfig, has_cpp: bool) -> String {
    if let Some(build) = &config.build {
        if let Some(compiler) = &build.compiler {
            return compiler.clone();
        }
    }

    if has_cpp {
        if let Ok(env_cxx) = std::env::var("CXX") {
            return env_cxx;
        }
    } else {
        if let Ok(env_cc) = std::env::var("CC") {
            return env_cc;
        }
    }

    if has_cpp {
        "clang++".to_string()
    } else {
        "clang".to_string()
    }
}

// --- Helper: Run Script (Cross Platform) ---
pub fn run_script(script: &str, project_dir: &Path) -> Result<()> {
    println!("   {} Running script: '{}'...", "📜".magenta(), script);
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", script])
            .current_dir(project_dir)
            .status()?
    } else {
        Command::new("sh")
            .args(&["-c", script])
            .current_dir(project_dir)
            .status()?
    };

    if !status.success() {
        return Err(anyhow::anyhow!("Script failed"));
    }
    Ok(())
}
