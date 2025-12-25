use crate::config::CxConfig;
use anyhow::{Context, Result};
use colored::*;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn generate_ide_config() -> Result<()> {
    println!("{} Setting up IDE configuration (VSCode)...", "⚙️".cyan());

    let vscode_dir = Path::new(".vscode");
    if !vscode_dir.exists() {
        fs::create_dir(vscode_dir).context("Failed to create .vscode directory")?;
    }

    // Load config to know binary name
    let config = crate::build::load_config().unwrap_or_else(|_| CxConfig {
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
    });

    let bin_name = if let Some(build) = &config.build {
        build.bin.clone().unwrap_or(config.package.name.clone())
    } else {
        config.package.name.clone()
    };

    let bin_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let bin_path_debug = format!("${{workspaceFolder}}/build/debug/{}{}", bin_name, bin_ext);

    // 1. tasks.json
    let tasks_json = json!({
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
    });
    write_json_if_missing(&vscode_dir.join("tasks.json"), &tasks_json)?;

    // 2. launch.json
    let launch_json = json!({
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
    });
    // Adjust type for non-windows if needed, but for now assuming user OS (Windows) from metadata
    // Or better, logic to detect or provide both?
    // Let's provide a generic configuration or one aimed at the current OS.
    // User is on Windows (MSVC usually), so `cppvsdbg` is safer. `cppdbg` (GDB) requires setup.
    write_json_if_missing(&vscode_dir.join("launch.json"), &launch_json)?;

    // 3. c_cpp_properties.json (IntelliSense)
    // We can try to infer include paths.
    // Global cache: ~/.cx/cache
    // Vendor: ./vendor
    let home_dir = dirs::home_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    let cache_dir = home_dir
        .join(".cx")
        .join("cache")
        .to_string_lossy()
        .replace("\\", "/");

    let cpp_properties = json!({
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
    });
    write_json_if_missing(&vscode_dir.join("c_cpp_properties.json"), &cpp_properties)?;

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
