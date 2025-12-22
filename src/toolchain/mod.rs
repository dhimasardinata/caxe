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

    // Detect fresh toolchain
    let toolchain = detect_toolchain(preferred)?;

    // Cache it
    if let Ok(toml_str) = toml::to_string_pretty(&toolchain) {
        let _ = std::fs::create_dir_all(cache_path.parent().unwrap());
        let _ = std::fs::write(&cache_path, toml_str);
    }

    Ok(toolchain)
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
