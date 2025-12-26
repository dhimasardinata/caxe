use crate::config::{CxConfig, Profile};
use crate::toolchain::{self, CompilerType, Toolchain, ToolchainError};
use anyhow::{Context, Result};
use colored::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

// --- Helper: Load Config with Profile Parsing ---
pub fn load_config() -> Result<CxConfig> {
    if !Path::new("cx.toml").exists() {
        return Err(anyhow::anyhow!(
            "cx.toml not found in current directory.\n\n\
            💡 Tip: Run 'cx init' to create one, or 'cx new <name>' for a new project."
        ));
    }
    let config_str =
        fs::read_to_string("cx.toml").context("Failed to read cx.toml - check file permissions")?;

    // Parse as raw TOML Value first to extract [profile:*] tables
    let raw_value: toml::Value = toml::from_str(&config_str)
        .context("Failed to parse cx.toml - check for syntax errors (missing quotes, brackets)")?;

    // Extract profiles from [profile:name] tables
    let mut profiles: HashMap<String, Profile> = HashMap::new();
    if let toml::Value::Table(root) = &raw_value {
        for (key, value) in root {
            if key.starts_with("profile:") {
                let profile_name = key.strip_prefix("profile:").unwrap().to_string();
                if let Ok(profile) = value.clone().try_into::<Profile>() {
                    profiles.insert(profile_name, profile);
                }
            }
        }
    }

    // Deserialize main config (profiles will be empty from flatten, we fill it manually)
    let mut config: CxConfig = toml::from_str(&config_str).context("Failed to parse cx.toml")?;

    // Merge extracted profiles into config
    config.profiles = profiles;

    // Deprecation warning for cflags
    if let Some(ref build_cfg) = config.build
        && build_cfg.uses_deprecated_cflags()
    {
        eprintln!(
            "   {} 'cflags' is deprecated, please use 'flags' instead in [build]",
            "⚠".yellow()
        );
    }

    Ok(config)
}

// --- Helper: Check if a command exists (for fallback only) ---
fn is_command_available(cmd: &str) -> bool {
    let mut command = Command::new(cmd);
    if cmd == "cl" || cmd == "cl.exe" {
        return command.arg("/?").output().is_ok();
    }
    command.arg("--version").output().is_ok()
}

// --- Helper: Get Toolchain (uses vswhere on Windows) ---
pub fn get_toolchain(config: &CxConfig, _has_cpp: bool) -> Result<Toolchain, ToolchainError> {
    // 1. Check if user specified a compiler in config
    let preferred = if let Some(build) = &config.build {
        if let Some(compiler) = &build.compiler {
            match compiler.to_lowercase().as_str() {
                "msvc" | "cl" | "cl.exe" => Some(CompilerType::MSVC),
                "clang-cl" | "clangcl" => Some(CompilerType::ClangCL),
                "clang" | "clang++" => Some(CompilerType::Clang),
                "gcc" | "g++" => Some(CompilerType::GCC),
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    // 2. Try to detect toolchain using proper discovery
    match toolchain::get_or_detect_toolchain(preferred, false) {
        Ok(tc) => {
            println!(
                "   {} Detected toolchain: {} ({})",
                "🔧".cyan(),
                tc.cxx_path.display(),
                tc.version
            );
            Ok(tc)
        }
        Err(e) => {
            // On Windows, show clear error message only for MSVC-related issues
            #[cfg(windows)]
            {
                let msg = format!("{}", e);
                // Don't show VS Install help for intentional non-MSVC compiler preferences
                if !msg.contains("Clang/GCC") {
                    println!("{} {}", "x".red(), e);
                    println!();
                    println!("{}:", "To fix this".bold());
                    println!("  1. Install Visual Studio Build Tools from:");
                    println!("     https://visualstudio.microsoft.com/visual-cpp-build-tools/");
                    println!("  2. Select 'Desktop development with C++' workload");
                    println!();
                }
            }
            Err(e)
        }
    }
}

// --- Helper: Legacy get_compiler for backward compatibility ---
pub fn get_compiler(config: &CxConfig, has_cpp: bool) -> String {
    // Try new toolchain detection first
    if let Ok(tc) = get_toolchain(config, has_cpp) {
        return tc.cxx_path.to_string_lossy().to_string();
    }

    // Fallback to old PATH-based detection (backward compatibility)
    println!(
        "   {} Falling back to PATH-based compiler detection",
        "⚠".yellow()
    );

    // Check Config
    if let Some(build) = &config.build
        && let Some(compiler) = &build.compiler
    {
        return compiler.clone();
    }

    // Check Env Vars
    if has_cpp {
        if let Ok(env_cxx) = std::env::var("CXX") {
            return env_cxx;
        }
    } else if let Ok(env_cc) = std::env::var("CC") {
        return env_cc;
    }

    // Auto-Detect from PATH
    if has_cpp {
        if is_command_available("clang++") {
            return "clang++".to_string();
        }
        if is_command_available("g++") {
            return "g++".to_string();
        }
        if cfg!(target_os = "windows") && is_command_available("cl") {
            return "cl".to_string();
        }
        "clang++".to_string()
    } else {
        if is_command_available("clang") {
            return "clang".to_string();
        }
        if is_command_available("gcc") {
            return "gcc".to_string();
        }
        if cfg!(target_os = "windows") && is_command_available("cl") {
            return "cl".to_string();
        }
        "clang".to_string()
    }
}

// --- Helper: Run Script (Cross Platform) ---
pub fn run_script(script: &str, project_dir: &Path) -> Result<()> {
    // Check if script file exists with .rhai extension
    if script.ends_with(".rhai") {
        let script_path = project_dir.join(script);
        if script_path.exists() {
            println!("   {} Running Rhai script: '{}'...", "📜".magenta(), script);
            let engine = rhai::Engine::new();
            engine
                .run_file(script_path)
                .map_err(|e| anyhow::anyhow!("Rhai script failed: {}", e))?;
            return Ok(());
        }
    }

    println!("   {} Running script: '{}'...", "📜".magenta(), script);
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", script])
            .current_dir(project_dir)
            .status()?
    } else {
        Command::new("sh")
            .args(["-c", script])
            .current_dir(project_dir)
            .status()?
    };

    if !status.success() {
        return Err(anyhow::anyhow!("Script failed"));
    }
    Ok(())
}

/// Get the MSVC-compatible standard flag for a given edition
/// MSVC uses /std: prefix and has different naming for newer standards
pub fn get_std_flag_msvc(edition: &str) -> String {
    let normalized = edition.to_lowercase().replace("c++", "").replace("c", "");

    match normalized.as_str() {
        // C standards
        "89" | "90" => "/std:c11".to_string(), // MSVC doesn't support c89/90, fall back to c11
        "99" => "/std:c11".to_string(),        // MSVC doesn't support c99, fall back to c11
        "11" if edition.starts_with("c") && !edition.contains("++") => "/std:c11".to_string(),
        "17" if edition.starts_with("c") && !edition.contains("++") => "/std:c17".to_string(),
        "23" if edition.starts_with("c") && !edition.contains("++") => "/std:clatest".to_string(),

        // C++ standards
        "98" | "03" => "/std:c++14".to_string(), // MSVC minimum is c++14
        "11" => "/std:c++14".to_string(),        // MSVC minimum is c++14
        "14" => "/std:c++14".to_string(),
        "17" => "/std:c++17".to_string(),
        "20" => "/std:c++20".to_string(),
        "23" => "/std:c++latest".to_string(), // MSVC uses c++latest for c++23
        "26" | "2c" => "/std:c++latest".to_string(), // Future standards
        "latest" => "/std:c++latest".to_string(),

        // If already in /std: format, pass through
        _ if edition.starts_with("/std:") => edition.to_string(),

        // Default: try to use as-is
        _ => format!("/std:{}", edition),
    }
}

/// Get the GCC/Clang-compatible standard flag for a given edition
/// GCC/Clang use -std= prefix
pub fn get_std_flag_gcc(edition: &str) -> String {
    let normalized = edition.to_lowercase();

    // If already in -std= format, extract the standard
    let edition_clean = normalized.strip_prefix("-std=").unwrap_or(&normalized);

    match edition_clean {
        // C standards - GCC/Clang support all of these
        "c89" | "c90" => "-std=c89".to_string(),
        "c99" => "-std=c99".to_string(),
        "c11" => "-std=c11".to_string(),
        "c17" | "c18" => "-std=c17".to_string(),
        "c23" | "c2x" => "-std=c23".to_string(),

        // C++ standards - GCC/Clang support all of these
        "c++98" | "c++03" => "-std=c++03".to_string(),
        "c++11" | "c++0x" => "-std=c++11".to_string(),
        "c++14" | "c++1y" => "-std=c++14".to_string(),
        "c++17" | "c++1z" => "-std=c++17".to_string(),
        "c++20" | "c++2a" => "-std=c++20".to_string(),
        "c++23" | "c++2b" => "-std=c++23".to_string(),
        "c++26" | "c++2c" => "-std=c++26".to_string(),

        // GNU extensions (supported by GCC and Clang)
        "gnu89" | "gnu90" => "-std=gnu89".to_string(),
        "gnu99" => "-std=gnu99".to_string(),
        "gnu11" => "-std=gnu11".to_string(),
        "gnu17" | "gnu18" => "-std=gnu17".to_string(),
        "gnu23" | "gnu2x" => "-std=gnu23".to_string(),
        "gnu++98" | "gnu++03" => "-std=gnu++03".to_string(),
        "gnu++11" | "gnu++0x" => "-std=gnu++11".to_string(),
        "gnu++14" | "gnu++1y" => "-std=gnu++14".to_string(),
        "gnu++17" | "gnu++1z" => "-std=gnu++17".to_string(),
        "gnu++20" | "gnu++2a" => "-std=gnu++20".to_string(),
        "gnu++23" | "gnu++2b" => "-std=gnu++23".to_string(),
        "gnu++26" | "gnu++2c" => "-std=gnu++26".to_string(),

        // Default: use as-is with -std= prefix
        _ => format!("-std={}", edition_clean),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_std_flag_msvc_cpp_standards() {
        assert_eq!(get_std_flag_msvc("c++14"), "/std:c++14");
        assert_eq!(get_std_flag_msvc("c++17"), "/std:c++17");
        assert_eq!(get_std_flag_msvc("c++20"), "/std:c++20");
        assert_eq!(get_std_flag_msvc("c++23"), "/std:c++latest");
    }

    #[test]
    fn test_get_std_flag_msvc_c_standards() {
        assert_eq!(get_std_flag_msvc("c11"), "/std:c11");
        assert_eq!(get_std_flag_msvc("c17"), "/std:c17");
        assert_eq!(get_std_flag_msvc("c23"), "/std:clatest");
    }

    #[test]
    fn test_get_std_flag_msvc_fallbacks() {
        // MSVC doesn't support old C standards, falls back
        assert_eq!(get_std_flag_msvc("c89"), "/std:c11");
        assert_eq!(get_std_flag_msvc("c99"), "/std:c11");
        // Old C++ standards fall back to c++14
        assert_eq!(get_std_flag_msvc("c++98"), "/std:c++14");
        assert_eq!(get_std_flag_msvc("c++11"), "/std:c++14");
    }

    #[test]
    fn test_get_std_flag_msvc_passthrough() {
        assert_eq!(get_std_flag_msvc("/std:c++20"), "/std:c++20");
    }

    #[test]
    fn test_get_std_flag_gcc_cpp_standards() {
        assert_eq!(get_std_flag_gcc("c++11"), "-std=c++11");
        assert_eq!(get_std_flag_gcc("c++14"), "-std=c++14");
        assert_eq!(get_std_flag_gcc("c++17"), "-std=c++17");
        assert_eq!(get_std_flag_gcc("c++20"), "-std=c++20");
        assert_eq!(get_std_flag_gcc("c++23"), "-std=c++23");
        assert_eq!(get_std_flag_gcc("c++26"), "-std=c++26");
    }

    #[test]
    fn test_get_std_flag_gcc_c_standards() {
        assert_eq!(get_std_flag_gcc("c89"), "-std=c89");
        assert_eq!(get_std_flag_gcc("c99"), "-std=c99");
        assert_eq!(get_std_flag_gcc("c11"), "-std=c11");
        assert_eq!(get_std_flag_gcc("c17"), "-std=c17");
        assert_eq!(get_std_flag_gcc("c23"), "-std=c23");
    }

    #[test]
    fn test_get_std_flag_gcc_gnu_extensions() {
        assert_eq!(get_std_flag_gcc("gnu++17"), "-std=gnu++17");
        assert_eq!(get_std_flag_gcc("gnu++20"), "-std=gnu++20");
        assert_eq!(get_std_flag_gcc("gnu11"), "-std=gnu11");
    }

    #[test]
    fn test_get_std_flag_gcc_aliases() {
        assert_eq!(get_std_flag_gcc("c++0x"), "-std=c++11");
        assert_eq!(get_std_flag_gcc("c++1y"), "-std=c++14");
        assert_eq!(get_std_flag_gcc("c++1z"), "-std=c++17");
        assert_eq!(get_std_flag_gcc("c++2a"), "-std=c++20");
        assert_eq!(get_std_flag_gcc("c++2b"), "-std=c++23");
        assert_eq!(get_std_flag_gcc("c2x"), "-std=c23");
    }

    #[test]
    fn test_get_std_flag_gcc_strip_prefix() {
        assert_eq!(get_std_flag_gcc("-std=c++20"), "-std=c++20");
    }
}
