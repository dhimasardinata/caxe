//! Toolchain discovery and management
//!
//! This module provides proper toolchain discovery without relying on PATH.
//! On Windows, it uses vswhere to find Visual Studio installations and
//! loads the vcvars environment in an isolated manner.

pub mod types;

#[cfg(windows)]
pub mod windows;

pub mod install; // Toolchain installer

pub use types::{CompilerType, Toolchain, ToolchainError};

use std::path::PathBuf;

/// Detect the best available toolchain for the current platform
pub fn detect_toolchain(preferred: Option<CompilerType>) -> Result<Toolchain, ToolchainError> {
    #[cfg(windows)]
    {
        windows::detect_toolchain(preferred)
    }

    #[cfg(not(windows))]
    {
        detect_unix_toolchain(preferred)
    }
}

/// Detect toolchain on Unix-like systems (Linux, macOS)
#[cfg(not(windows))]
fn detect_unix_toolchain(preferred: Option<CompilerType>) -> Result<Toolchain, ToolchainError> {
    use std::process::Command;

    // Try clang++ first, then g++
    let compilers = match preferred {
        Some(CompilerType::GCC) => {
            vec![("g++", CompilerType::GCC), ("clang++", CompilerType::Clang)]
        }
        _ => vec![("clang++", CompilerType::Clang), ("g++", CompilerType::GCC)],
    };

    for (cmd, compiler_type) in compilers {
        if let Ok(output) = Command::new("which").arg(cmd).output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let cxx_path = PathBuf::from(&path_str);

                // Get version
                let version = Command::new(cmd)
                    .arg("--version")
                    .output()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .next()
                            .unwrap_or("unknown")
                            .to_string()
                    })
                    .unwrap_or_else(|_| "unknown".to_string());

                return Ok(Toolchain::new_simple(compiler_type, cxx_path, version));
            }
        }
    }

    Err(ToolchainError::NotFound(
        "No C++ compiler found. Please install clang or gcc.".to_string(),
    ))
}

/// Get a cached toolchain or detect a new one
pub fn get_or_detect_toolchain(
    preferred: Option<CompilerType>,
    force_detect: bool,
) -> Result<Toolchain, ToolchainError> {
    // 1. First, check user selection cache (from `cx toolchain select`)
    let selection_path = get_user_selection_path();
    if !force_detect
        && selection_path.exists()
        && let Ok(contents) = std::fs::read_to_string(&selection_path)
    {
        // Parse the selection file to get compiler type, path, and source
        let mut selected_type: Option<CompilerType> = None;
        let mut selected_path: Option<PathBuf> = None;
        let mut selected_source: Option<String> = None;

        for line in contents.lines() {
            if line.starts_with("compiler_type") {
                if line.contains("\"MSVC\"") {
                    selected_type = Some(CompilerType::MSVC);
                } else if line.contains("\"ClangCL\"") {
                    selected_type = Some(CompilerType::ClangCL);
                } else if line.contains("\"Clang\"") {
                    selected_type = Some(CompilerType::Clang);
                } else if line.contains("\"GCC\"") {
                    selected_type = Some(CompilerType::GCC);
                }
            }
            if line.starts_with("path") {
                // Extract path from: path = "C:\..."
                if let Some(start) = line.find('"')
                    && let Some(end) = line.rfind('"')
                    && start < end
                {
                    selected_path = Some(PathBuf::from(&line[start + 1..end]));
                }
            }
            if line.starts_with("source") {
                // Extract source from: source = "Visual Studio Build Tools 2026"
                if let Some(start) = line.find('"')
                    && let Some(end) = line.rfind('"')
                    && start < end
                {
                    selected_source = Some(line[start + 1..end].to_string());
                }
            }
        }

        // If user has a selection and it matches any preference (or no preference)
        if let (Some(sel_type), Some(path)) = (&selected_type, &selected_path) {
            let matches_preference = match &preferred {
                None => true,
                Some(pref) => pref == sel_type,
            };

            if matches_preference && path.exists() {
                // For MSVC/ClangCL, need to detect from specific VS installation
                #[cfg(windows)]
                {
                    if let Some(ref source) = selected_source
                        && let Ok(toolchain) =
                            windows::detect_toolchain_from_source(sel_type.clone(), source)
                    {
                        return Ok(toolchain);
                    }
                }

                // For GCC or if source detection fails, try direct path detection
                if sel_type == &CompilerType::GCC {
                    let version = std::process::Command::new(path)
                        .arg("--version")
                        .output()
                        .map(|o| {
                            String::from_utf8_lossy(&o.stdout)
                                .lines()
                                .next()
                                .unwrap_or("unknown")
                                .to_string()
                        })
                        .unwrap_or_else(|_| "unknown".to_string());

                    return Ok(Toolchain {
                        compiler_type: CompilerType::GCC,
                        cc_path: path.with_file_name("gcc.exe"),
                        cxx_path: path.clone(),
                        linker_path: PathBuf::new(),
                        version,
                        msvc_toolset_version: None,
                        windows_sdk_version: None,
                        vs_install_path: None,
                        env_vars: std::collections::HashMap::new(),
                    });
                }
            }
        }
    }

    // 2. Fall back to auto-detected cache
    let cache_path = get_toolchain_cache_path();

    // Try to load from cache first
    if !force_detect
        && cache_path.exists()
        && let Ok(contents) = std::fs::read_to_string(&cache_path)
        && let Ok(cached) = toml::from_str::<Toolchain>(&contents)
    {
        // Verify the cached toolchain still exists AND matches preference
        if cached.cxx_path.exists() {
            // If user has a preference, cache must match it
            let matches_preference = match &preferred {
                None => true, // No preference, any cached toolchain is fine
                Some(pref) => *pref == cached.compiler_type,
            };

            if matches_preference {
                return Ok(cached);
            }
        }
    }

    // 3. Detect fresh toolchain
    let toolchain = detect_toolchain(preferred)?;

    // Cache it
    if let Ok(toml_str) = toml::to_string_pretty(&toolchain) {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&cache_path, toml_str);
    }

    Ok(toolchain)
}

/// Get the path to the user selection file (from `cx toolchain select`)
fn get_user_selection_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cx")
        .join("toolchain-selection.toml")
}

/// Get the path to the toolchain cache file
fn get_toolchain_cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cx")
        .join("toolchain.toml")
}

/// Clear the toolchain cache
#[allow(dead_code)]
pub fn clear_toolchain_cache() {
    let cache_path = get_toolchain_cache_path();
    let _ = std::fs::remove_file(cache_path);
}
