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

use std::path::{Path, PathBuf};

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
    let selection_path = get_user_selection_path();
    let cache_path = get_toolchain_cache_path();
    if let Some(selected) = load_user_selected_toolchain(&preferred, force_detect, &selection_path)
    {
        return Ok(selected);
    }

    if let Some(cached) = load_cached_toolchain(&preferred, force_detect, &cache_path) {
        return Ok(cached);
    }

    let toolchain = detect_toolchain(preferred)?;
    cache_toolchain(&cache_path, &toolchain);
    Ok(toolchain)
}

#[derive(Default)]
struct ParsedSelection {
    compiler_type: Option<CompilerType>,
    path: Option<PathBuf>,
    source: Option<String>,
}

fn matches_preference(preferred: &Option<CompilerType>, compiler_type: &CompilerType) -> bool {
    preferred
        .as_ref()
        .map(|pref| pref == compiler_type)
        .unwrap_or(true)
}

fn extract_quoted_value(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let end = line.rfind('"')?;
    (start < end).then(|| line[start + 1..end].to_string())
}

fn parse_user_selection(contents: &str) -> ParsedSelection {
    let mut parsed = ParsedSelection::default();

    for line in contents.lines() {
        if line.starts_with("compiler_type") {
            parsed.compiler_type = match () {
                _ if line.contains("\"MSVC\"") => Some(CompilerType::MSVC),
                _ if line.contains("\"ClangCL\"") => Some(CompilerType::ClangCL),
                _ if line.contains("\"Clang\"") => Some(CompilerType::Clang),
                _ if line.contains("\"GCC\"") => Some(CompilerType::GCC),
                _ => None,
            };
            continue;
        }

        if line.starts_with("path") {
            parsed.path = extract_quoted_value(line).map(PathBuf::from);
            continue;
        }

        if line.starts_with("source") {
            parsed.source = extract_quoted_value(line);
        }
    }

    parsed
}

fn gcc_toolchain_from_selected_path(path: &Path) -> Toolchain {
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

    Toolchain {
        compiler_type: CompilerType::GCC,
        cc_path: path.with_file_name("gcc.exe"),
        cxx_path: path.to_path_buf(),
        linker_path: PathBuf::new(),
        version,
        msvc_toolset_version: None,
        windows_sdk_version: None,
        vs_install_path: None,
        env_vars: std::collections::HashMap::new(),
    }
}

#[cfg(windows)]
fn toolchain_from_source(selection: &ParsedSelection) -> Option<Toolchain> {
    let sel_type = selection.compiler_type.clone()?;
    let source = selection.source.as_ref()?;
    windows::detect_toolchain_from_source(sel_type, source).ok()
}

#[cfg(not(windows))]
fn toolchain_from_source(_selection: &ParsedSelection) -> Option<Toolchain> {
    None
}

fn load_user_selected_toolchain(
    preferred: &Option<CompilerType>,
    force_detect: bool,
    selection_path: &Path,
) -> Option<Toolchain> {
    if force_detect || !selection_path.exists() {
        return None;
    }

    let contents = std::fs::read_to_string(selection_path).ok()?;
    let selection = parse_user_selection(&contents);
    let selected_type = selection.compiler_type.as_ref()?;
    let selected_path = selection.path.as_ref()?;

    if !matches_preference(preferred, selected_type) || !selected_path.exists() {
        return None;
    }

    if let Some(toolchain) = toolchain_from_source(&selection) {
        return Some(toolchain);
    }

    (selected_type == &CompilerType::GCC).then(|| gcc_toolchain_from_selected_path(selected_path))
}

fn load_cached_toolchain(
    preferred: &Option<CompilerType>,
    force_detect: bool,
    cache_path: &Path,
) -> Option<Toolchain> {
    if force_detect || !cache_path.exists() {
        return None;
    }

    let contents = std::fs::read_to_string(cache_path).ok()?;
    let cached = toml::from_str::<Toolchain>(&contents).ok()?;
    if cached.cxx_path.exists() && matches_preference(preferred, &cached.compiler_type) {
        Some(cached)
    } else {
        None
    }
}

fn cache_toolchain(cache_path: &Path, toolchain: &Toolchain) {
    if let Ok(toml_str) = toml::to_string_pretty(toolchain) {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(cache_path, toml_str);
    }
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
