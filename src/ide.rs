//! IDE configuration generators.
//!
//! This module provides the `cx setup-ide` command which generates configuration
//! files for development environments, currently focused on Visual Studio Code.
//!
//! ## Generated Files
//!
//! - `.vscode/tasks.json` - Build and test tasks
//! - `.vscode/launch.json` - Debug configuration
//! - `.vscode/c_cpp_properties.json` - IntelliSense settings

use crate::config::CxConfig;
use anyhow::{Context, Result};
use colored::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn default_config() -> CxConfig {
    CxConfig {
        package: crate::config::PackageConfig {
            name: "app".to_string(),
            version: "0.1.0".to_string(),
            edition: "c++17".to_string(),
        },
        build: None,
        dependencies: None,
        scripts: None,
        test: None,
        workspace: None,
        arduino: None,
        profiles: std::collections::HashMap::new(),
    }
}

fn load_or_default_config() -> CxConfig {
    crate::build::load_config().unwrap_or_else(|_| default_config())
}

fn ensure_vscode_dir() -> Result<PathBuf> {
    let vscode_dir = PathBuf::from(".vscode");
    if !vscode_dir.exists() {
        fs::create_dir(&vscode_dir).context("Failed to create .vscode directory")?;
    }
    Ok(vscode_dir)
}

fn debug_binary_path(config: &CxConfig) -> String {
    let debug_bin_rel = crate::build::artifact_bin_path(config, false, false)
        .to_string_lossy()
        .replace('\\', "/");
    format!("${{workspaceFolder}}/{}", debug_bin_rel)
}

fn tasks_json() -> serde_json::Value {
    json!({
        "version": "2.0.0",
        "tasks": [
            {
                "label": "Build Debug",
                "type": "shell",
                "command": "cx build",
                "group": {
                    "kind": "build",
                    "isDefault": true
                },
                "problemMatcher": ["$gcc"]
            },
            {
                "label": "Build Release",
                "type": "shell",
                "command": "cx build --release",
                "group": "build",
                "problemMatcher": ["$gcc"]
            },
            {
                "label": "Test",
                "type": "shell",
                "command": "cx test",
                "group": "test",
                "problemMatcher": []
            }
        ]
    })
}

fn launch_json(bin_path_debug: &str) -> serde_json::Value {
    json!({
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Debug (Caxe)",
                "type": "cppvsdbg", // Default for Windows (MSVC), cppdbg for GDB/LLDB
                "request": "launch",
                "program": bin_path_debug,
                "args": [],
                "stopAtEntry": false,
                "cwd": "${workspaceFolder}",
                "environment": [],
                "console": "integratedTerminal",
                "preLaunchTask": "Build Debug"
            }
        ]
    })
}

fn cpp_properties_json() -> serde_json::Value {
    let home_dir = dirs::home_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    let cache_dir = home_dir
        .join(".cx")
        .join("cache")
        .to_string_lossy()
        .replace("\\", "/");

    json!({
        "configurations": [
            {
                "name": "Win32",
                "includePath": [
                    "${workspaceFolder}/**",
                    "${workspaceFolder}/include",
                    format!("{}/**", cache_dir),
                    "${workspaceFolder}/vendor/**"
                ],
                "defines": [
                    "_DEBUG",
                    "UNICODE",
                    "_UNICODE"
                ],
                "windowsSdkVersion": "10.0.19041.0",
                "compilerPath": "cl.exe", // Assume MSVC on Windows
                "cStandard": "c17",
                "cppStandard": "c++17",
                "intelliSenseMode": "windows-msvc-x64"
            }
        ],
        "version": 4
    })
}

pub fn generate_ide_config() -> Result<()> {
    println!("{} Setting up IDE configuration (VSCode)...", "⚙️".cyan());

    let vscode_dir = ensure_vscode_dir()?;
    let config = load_or_default_config();
    let bin_path_debug = debug_binary_path(&config);

    write_json_if_missing(&vscode_dir.join("tasks.json"), &tasks_json())?;
    write_json_if_missing(
        &vscode_dir.join("launch.json"),
        &launch_json(&bin_path_debug),
    )?;
    write_json_if_missing(
        &vscode_dir.join("c_cpp_properties.json"),
        &cpp_properties_json(),
    )?;

    println!("{} VSCode configuration generated in .vscode/", "✓".green());
    Ok(())
}

fn write_json_if_missing(path: &Path, content: &serde_json::Value) -> Result<()> {
    if path.exists() {
        println!(
            "   {} Skipping existing {}",
            "!".yellow(),
            path.file_name().unwrap_lossy().to_string_lossy()
        );
        return Ok(());
    }
    let formatted = serde_json::to_string_pretty(content)?;
    fs::write(path, formatted).context(format!("Failed to write {:?}", path))?;
    println!(
        "   {} Created {}",
        "+".green(),
        path.file_name().unwrap_lossy().to_string_lossy()
    );
    Ok(())
}

trait UnwrapLossy {
    fn unwrap_lossy(&self) -> std::ffi::OsString;
}
impl UnwrapLossy for Option<&std::ffi::OsStr> {
    fn unwrap_lossy(&self) -> std::ffi::OsString {
        self.unwrap_or(std::ffi::OsStr::new("")).to_os_string()
    }
}
