//! Core compilation engine.
//!
//! This is the heart of caxe - the parallel compilation system that transforms
//! C/C++ source files into executables.
//!
//! ## Features
//!
//! - Lock-free parallel compilation using rayon
//! - Incremental builds (only recompile changed files)
//! - Compile commands JSON generation for IDE integration
//! - Chrome trace profiling output
//! - LTO and sanitizer support

use super::utils::{get_compiler, get_std_flag_gcc, get_std_flag_msvc, load_config, run_script};
use crate::config::CxConfig;
use crate::deps;
use crate::ui;
use anyhow::{Context, Result};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use walkdir::WalkDir;

#[derive(serde::Serialize)]
struct TraceEvent {
    name: String,
    cat: String,
    ph: String,
    ts: u128,
    dur: u128,
    pid: u32,
    tid: usize,
}

#[derive(Default, Clone)]
pub struct BuildOptions {
    pub release: bool,
    pub verbose: bool,
    pub dry_run: bool,
    pub enable_profile: bool,
    pub wasm: bool,
    pub lto: bool,
    pub sanitize: Option<String>,
    /// Named profile for cross-compilation (e.g., "esp32", "linux-arm64")
    pub profile: Option<String>,
}

// --- Helper: Check Dependencies (.d file or .json for MSVC) ---
fn check_dependencies(obj_path: &Path, src_path: &Path) -> Result<bool> {
    // 1. Check for MSVC JSON dependencies first
    // Actually typically /sourceDependencies foo.json -> foo.json.
    // We will name it <obj>.json explicitly.
    let json_path = PathBuf::from(format!("{}.json", obj_path.display()));

    if json_path.exists() {
        let content = fs::read_to_string(&json_path)?;
        // Parse JSON: {"Data": {"Source": "...", "Includes": ["..."]}}
        let json: serde_json::Value = serde_json::from_str(&content)?;

        // Check Includes
        if let Some(includes) = json.pointer("/Data/Includes")
            && let Some(arr) = includes.as_array()
        {
            let obj_mtime = fs::metadata(obj_path)?.modified()?;

            for include in arr {
                if let Some(path_str) = include.as_str() {
                    let dep_path = Path::new(path_str);
                    if dep_path.exists() {
                        let dep_mtime = fs::metadata(dep_path)?.modified()?;
                        if dep_mtime > obj_mtime {
                            return Ok(true); // Dependency newer
                        }
                    }
                }
            }
            // Also check source itself just in case? Usually built-in but good to double check.
            let src_mtime = fs::metadata(src_path)?.modified()?;
            if src_mtime > obj_mtime {
                return Ok(true);
            }
            return Ok(false); // All clean
        }
    }

    // 2. Check for GCC/Clang .d file
    let d_path = obj_path.with_extension("d");

    // Fallback: If no dependency file, check if source is newer than object
    if !d_path.exists() {
        if !obj_path.exists() {
            return Ok(true);
        }
        let src_mtime = fs::metadata(src_path)?.modified()?;
        let obj_mtime = fs::metadata(obj_path)?.modified()?;
        return Ok(src_mtime > obj_mtime);
    }

    let dep_content = fs::read_to_string(&d_path)?;
    // Handle line continuations
    let content_flat = dep_content.replace("\\\n", " ").replace("\\\r\n", " ");

    // Format is usually: "objfile.o: src.c header.h ..."
    if let Some(deps_part) = content_flat.split_once(':') {
        let deps_str = deps_part.1;
        let obj_mtime = fs::metadata(obj_path)?.modified()?;

        for dep in deps_str.split_whitespace() {
            let dep_path = Path::new(dep);
            if dep_path.exists() {
                let dep_mtime = fs::metadata(dep_path)?.modified()?;
                if dep_mtime > obj_mtime {
                    return Ok(true); // Dependency is newer
                }
            }
        }
    }

    Ok(false) // Up to date
}

// --- CORE: Build Project ---
pub fn build_project(config: &CxConfig, options: &BuildOptions) -> Result<bool> {
    let release = options.release;
    let verbose = options.verbose;
    let dry_run = options.dry_run;
    let enable_profile = options.enable_profile;
    let wasm = options.wasm;
    let lto = options.lto;
    let sanitize = options.sanitize.clone();
    let start_time = Instant::now();

    // --- Profile Resolution with Inheritance ---
    // Clone config for potential modification based on selected profile
    let mut effective_config = config.clone();

    if let Some(profile_name) = &options.profile {
        // Look up the profile in config.profiles
        if let Some(profile) = config.profiles.get(profile_name) {
            println!(
                "   {} Using profile: {}",
                "🎯".magenta(),
                profile_name.cyan().bold()
            );

            // Resolve base profile first (inheritance)
            let mut resolved_flags: Vec<String> = Vec::new();
            let mut resolved_libs: Vec<String> = Vec::new();
            let mut resolved_compiler: Option<String> = None;

            if let Some(base_name) = &profile.base {
                // Handle built-in profiles (release/debug) or user-defined profiles
                if base_name == "release" {
                    // Release implies optimizations, but we handle that via options.release
                    // Just note it for verbosity
                    if verbose {
                        println!("      {} Inheriting from: {}", "└─".dimmed(), base_name);
                    }
                } else if base_name == "debug" {
                    // Debug is default, nothing special needed
                    if verbose {
                        println!("      {} Inheriting from: {}", "└─".dimmed(), base_name);
                    }
                } else if let Some(base_profile) = config.profiles.get(base_name) {
                    // Inherit from another user-defined profile
                    if verbose {
                        println!("      {} Inheriting from: {}", "└─".dimmed(), base_name);
                    }
                    if let Some(ref flags) = base_profile.flags {
                        resolved_flags.extend(flags.clone());
                    }
                    if let Some(ref libs) = base_profile.libs {
                        resolved_libs.extend(libs.clone());
                    }
                    if let Some(ref compiler) = base_profile.compiler {
                        resolved_compiler = Some(compiler.clone());
                    }
                }
            }

            // Apply this profile's settings (override base)
            if let Some(ref flags) = profile.flags {
                resolved_flags.extend(flags.clone());
            }
            if let Some(ref libs) = profile.libs {
                resolved_libs.extend(libs.clone());
            }
            if let Some(ref compiler) = profile.compiler {
                resolved_compiler = Some(compiler.clone());
            }

            // Apply resolved values to effective_config
            let build_cfg = effective_config.build.get_or_insert_with(Default::default);

            // Merge flags into build config
            if !resolved_flags.is_empty() {
                let existing = build_cfg.flags.get_or_insert_with(Vec::new);
                existing.extend(resolved_flags);
            }

            // Override libs
            if !resolved_libs.is_empty() {
                let existing = build_cfg.libs.get_or_insert_with(Vec::new);
                existing.extend(resolved_libs);
            }

            // Override compiler
            if let Some(compiler) = resolved_compiler {
                build_cfg.compiler = Some(compiler);
            }

            // Apply bin name override
            if let Some(ref bin) = profile.bin {
                build_cfg.bin = Some(bin.clone());
            }
        } else {
            return Err(anyhow::anyhow!(
                "Profile '{}' not found in cx.toml. Available profiles: {:?}",
                profile_name,
                config.profiles.keys().collect::<Vec<_>>()
            ));
        }
    }

    // Use effective_config from now on
    let config = &effective_config;
    let current_dir = std::env::current_dir()?;

    // Dry-run or Verbose header with modern box styling
    let show_details = verbose || dry_run;

    if show_details {
        let edition_str = if config.package.edition.is_empty() {
            "c++17"
        } else {
            &config.package.edition
        };
        let profile_str = if release { "release" } else { "debug" };
        let compiler_str = if wasm {
            "em++"
        } else {
            config
                .build
                .as_ref()
                .and_then(|b| b.compiler.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("auto")
        };

        println!();
        let mode = if dry_run {
            "DRY RUN".yellow().bold()
        } else {
            "BUILD".cyan().bold()
        };
        let icon = if dry_run { "🔍" } else { "🔧" };
        println!("  {} {}", icon, mode);

        let mut table = ui::Table::new(&["Setting", "Value"]);

        let pkg_value = format!("{} v{}", config.package.name, config.package.version);
        table.add_row(vec![
            "Package".dimmed().to_string(),
            pkg_value.cyan().to_string(),
        ]);

        let profile_colored = if release {
            profile_str.green()
        } else {
            profile_str.yellow()
        };
        table.add_row(vec![
            "Profile".dimmed().to_string(),
            profile_colored.to_string(),
        ]);

        table.add_row(vec![
            "Edition".dimmed().to_string(),
            edition_str.to_string(),
        ]);
        table.add_row(vec![
            "Compiler".dimmed().to_string(),
            compiler_str.cyan().to_string(),
        ]);
        if wasm {
            table.add_row(vec![
                "Target".dimmed().to_string(),
                "WASM (Emscripten)".magenta().to_string(),
            ]);
        }
        if lto {
            table.add_row(vec![
                "LTO".dimmed().to_string(),
                "Enabled".green().bold().to_string(),
            ]);
        }
        if let Some(san) = &sanitize {
            table.add_row(vec![
                "Sanitizers".dimmed().to_string(),
                san.yellow().bold().to_string(),
            ]);
        }

        table.print();
        println!();
    }

    // 1. Pre-build Script
    if let Some(scripts) = &config.scripts
        && let Some(pre) = &scripts.pre_build
    {
        if verbose {
            println!("{} Running pre-build script: {}", "→".blue(), pre);
        }
        if let Err(e) = run_script(pre, &current_dir) {
            println!("{} Pre-build script failed: {}", "x".red(), e);
            return Ok(false);
        }
    }

    // 2. Setup Directories
    let profile = if release { "release" } else { "debug" };
    let build_dir = Path::new(".cx").join("build").join(profile);
    let obj_dir = build_dir.join("obj");
    fs::create_dir_all(&obj_dir)?;

    let bin_basename = if let Some(build_cfg) = &config.build {
        build_cfg.bin.clone().unwrap_or(config.package.name.clone())
    } else {
        config.package.name.clone()
    };

    let bin_name = if wasm {
        format!("{}.html", bin_basename)
    } else if cfg!(target_os = "windows") {
        format!("{}.exe", bin_basename)
    } else {
        bin_basename
    };

    let output_bin = build_dir.join(&bin_name);

    if verbose {
        println!("{}", "Paths:".bold());
        println!("  Output: {}", output_bin.display().to_string().cyan());
        println!("  Objects: {}", obj_dir.display().to_string().dimmed());
        println!();
    }

    // 3. Fetch Dependencies
    let mut include_paths = Vec::new();
    let mut extra_cflags = Vec::new();
    let mut dep_libs = Vec::new();

    if let Some(deps) = &config.dependencies
        && !deps.is_empty()
    {
        let (paths, cflags, libs) = deps::fetch_dependencies(deps)?;
        include_paths = paths;
        extra_cflags = cflags;
        dep_libs = libs;
    }

    // 4. Collect Source Files
    let mut source_files = Vec::new();
    let mut has_cpp = false;

    if let Some(build_cfg) = &config.build
        && let Some(explicit_sources) = &build_cfg.sources
    {
        for src in explicit_sources {
            let path = Path::new(src);
            if path.exists() {
                if let Some(ext) = path.extension() {
                    let s = ext.to_string_lossy();
                    if s != "c" {
                        has_cpp = true;
                    }
                    source_files.push(path.to_owned());
                }
            } else {
                println!("{} Source file not found: {}", "!".yellow(), src);
            }
        }
    } else {
        for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let s = ext.to_string_lossy();
                if ["cpp", "cc", "cxx", "c"].contains(&s.as_ref()) {
                    if s != "c" {
                        has_cpp = true;
                    }
                    source_files.push(path.to_owned());
                }
            }
        }
    }

    if source_files.is_empty() {
        println!("{} No source files found.", "!".yellow());
        return Ok(false);
    }

    // Get toolchain with environment variables
    let toolchain = if wasm {
        None
    } else {
        super::utils::get_toolchain(config, has_cpp).ok()
    };

    let compiler = if wasm {
        "em++".to_string()
    } else if let Some(ref tc) = toolchain {
        tc.cxx_path.to_string_lossy().to_string()
    } else {
        get_compiler(config, has_cpp)
    };

    // Helper to check for CCache
    let ccache_prefix = if !wasm {
        if Command::new("ccache").arg("--version").output().is_ok() {
            Some("ccache")
        } else if Command::new("sccache").arg("--version").output().is_ok() {
            Some("sccache")
        } else {
            None
        }
    } else {
        None // don't use ccache with emscripten unless configured carefully
    };

    if wasm {
        // Simple check if em++ exists
        if Command::new(&compiler).arg("--version").output().is_err() {
            println!("{} Emscripten (em++) not found in PATH.", "x".red());
            println!("   Please install Emscripten SDK.");
            return Ok(false);
        }
    }

    let is_msvc = compiler.contains("cl.exe") || compiler == "cl";
    // let is_clang = compiler.contains("clang");
    // let is_gcc = compiler.contains("g++") || compiler.contains("gcc");

    let current_dir_str = current_dir.to_string_lossy().to_string();

    // Verbose: Show toolchain info
    if verbose {
        println!("{}", "Toolchain:".bold());
        println!("  Compiler: {}", compiler.cyan());
        if let Some(cc) = ccache_prefix {
            println!("  Wrapper: {}", cc.yellow().bold());
        }
        println!(
            "  Type: {}",
            if is_msvc {
                "MSVC".yellow()
            } else {
                "GCC/Clang".green()
            }
        );
        if let Some(ref tc) = toolchain {
            println!("  Source: Detected via vswhere/explicit config");
            if !tc.env_vars.is_empty() {
                println!("  Env vars: {} injected", tc.env_vars.len());
            }
        } else {
            println!("  Source: PATH fallback");
        }
        println!();
    }

    // Clone env_vars for use in parallel compilation
    let toolchain_env: std::collections::HashMap<String, String> = toolchain
        .as_ref()
        .map(|tc| tc.env_vars.clone())
        .unwrap_or_default();

    // Prepare Common Flags (Includes)
    let mut common_flags = Vec::new();
    for path in &include_paths {
        if is_msvc {
            common_flags.push(format!("/I{}", path.display()));
        } else {
            common_flags.push(format!("-I{}", path.display()));
        }
    }

    // LTO Flags
    if lto {
        if is_msvc {
            common_flags.push("/GL".to_string()); // Whole Program Optimization (Compile)
        } else {
            common_flags.push("-flto".to_string());
        }
    }

    // Sanitizer Flags (GCC/Clang only mostly)
    if let Some(checks) = &sanitize {
        if !is_msvc {
            common_flags.push(format!("-fsanitize={}", checks));
            common_flags.push("-fno-omit-frame-pointer".to_string()); // Good practice for sanitizers
        } else {
            // Very limited MSVC AddressSanitizer support exists in newer VS, but args differ.
            // For now, warn user it might not work as expected or requires specific VS version.
            common_flags.push(format!("/fsanitize={}", checks)); // Recent MSVC uses this syntax
        }
    }

    common_flags.extend(extra_cflags.clone());

    // Verbose: Show include paths and flags
    if verbose && !include_paths.is_empty() {
        println!("{}", "Include Paths:".bold());
        for path in &include_paths {
            println!("  -I {}", path.display().to_string().dimmed());
        }
        println!();
    }

    // 5. Dry-run: Show compile commands that would be executed
    if dry_run {
        println!("{}", "Compile:".bold());
        for src_path in &source_files {
            let stem = src_path
                .file_stem()
                .unwrap_or(src_path.as_os_str())
                .to_string_lossy();
            let obj_ext = if is_msvc { "obj" } else { "o" };
            let obj_path = obj_dir.join(format!("{}.{}", stem, obj_ext));

            // Shorter, cleaner format
            let short_compiler = Path::new(&compiler)
                .file_name()
                .unwrap_or(compiler.as_ref())
                .to_string_lossy();

            let prefix = ccache_prefix.map(|c| format!("{} ", c)).unwrap_or_default();

            let cmd = format!(
                "  → {}{} -c {} → {}",
                prefix,
                short_compiler,
                src_path.display(),
                obj_path
                    .file_name()
                    .unwrap_or(obj_path.as_os_str())
                    .to_string_lossy()
            );
            println!("{}", cmd.dimmed());
        }

        // Show link command
        println!("\n{}", "Link:".bold());
        let short_compiler = Path::new(&compiler)
            .file_name()
            .unwrap_or(compiler.as_ref())
            .to_string_lossy();
        let obj_count = source_files.len();
        let bin_name = output_bin
            .file_name()
            .unwrap_or(output_bin.as_os_str())
            .to_string_lossy();
        println!(
            "  → {} [{} object(s)] → {}",
            short_compiler,
            obj_count,
            bin_name.cyan()
        );

        println!();
        println!("  {} {}", "✓".green(), "Dry run complete".bold());
        println!("  {}", "No commands were executed.".dimmed());
        return Ok(true);
    }

    // 5b. Precompiled Headers (PCH)
    let mut pch_args = Vec::new();
    if let Some(build_cfg) = &config.build
        && let Some(pch_str) = &build_cfg.pch
    {
        let pch_source = Path::new(pch_str);
        if !pch_source.exists() {
            println!("{} PCH file not found: {}", "!".yellow(), pch_str);
        } else {
            let pch_name = pch_source.file_name().unwrap_or_default().to_string_lossy();

            if is_msvc {
                let pch_out = obj_dir.join(format!("{}.pch", pch_name));
                // Check mtime
                let need_pch = !pch_out.exists()
                    || pch_source
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        > pch_out
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                if need_pch {
                    println!("{} Compiling PCH (MSVC)...", "⚙".cyan());
                    let mut cmd = Command::new(&compiler);
                    cmd.args(["/nologo", "/c", "/EHsc"]);
                    cmd.arg("/Yc"); // Create PCH
                    cmd.arg(format!("/Fp{}", pch_out.display()));
                    cmd.arg(pch_source);
                    // Add includes/defines
                    cmd.args(&common_flags);
                    cmd.arg(format!(
                        "/Fo{}",
                        obj_dir.join(format!("{}.obj", pch_name)).display()
                    ));

                    // Envs
                    if !toolchain_env.is_empty() {
                        cmd.envs(&toolchain_env);
                    }

                    let out = cmd.output()?;
                    if !out.status.success() {
                        return Err(anyhow::anyhow!(
                            "Failed to compile PCH: {}",
                            String::from_utf8_lossy(&out.stderr)
                        ));
                    }
                }
                // Use PCH flags for other files
                pch_args.push(format!("/Yu{}", pch_name));
                pch_args.push(format!("/Fp{}", pch_out.display()));

                // MSVC requires the object file of the PCH to be linked too!
                dep_libs.push(
                    obj_dir
                        .join(format!("{}.obj", pch_name))
                        .to_string_lossy()
                        .to_string(),
                );
            } else {
                // GCC / Clang
                // Output: build/header.hpp.gch
                let pch_out = obj_dir.join(format!("{}.gch", pch_name));
                let need_pch = !pch_out.exists()
                    || pch_source
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        > pch_out
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                if need_pch {
                    println!("{} Compiling PCH (GCC/Clang)...", "⚙".cyan());

                    // Handle ccache prefix for PCH? Usually safe.
                    let mut cmd = if let Some(wrapper) = ccache_prefix {
                        let mut c = Command::new(wrapper);
                        c.arg(&compiler);
                        c
                    } else {
                        Command::new(&compiler)
                    };

                    cmd.args(["-c"]);
                    cmd.arg(pch_source);
                    cmd.arg("-o");
                    cmd.arg(&pch_out);
                    cmd.args(&common_flags);
                    cmd.arg(format!("-std={}", config.package.edition));

                    let out = cmd.output()?;
                    if !out.status.success() {
                        return Err(anyhow::anyhow!(
                            "Failed to compile PCH: {}",
                            String::from_utf8_lossy(&out.stderr)
                        ));
                    }
                }
                // Use PCH
                // For GCC to find "header.hpp.gch" when user asks for "header.hpp",
                // we need "-I build/".
                pch_args.push(format!("-I{}", obj_dir.display()));
            }
        }
    }

    // Profiling Setup
    let trace_events = if enable_profile {
        Some(Arc::new(Mutex::new(Vec::new())))
    } else {
        None
    };
    let build_start_time = Instant::now();

    // 6. Parallel Compilation (Lock-Free Optimization)
    let spinner_style = ProgressStyle::default_spinner()
        .template("{spinner:.cyan} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        .progress_chars("█▓░");

    let pb = ProgressBar::new(source_files.len() as u64);
    pb.set_style(spinner_style);
    pb.set_message("Compiling...");

    let results: Vec<(PathBuf, serde_json::Value)> = source_files
        .par_iter()
        .map(|src_path| -> Result<(PathBuf, serde_json::Value)> {
            let stem = src_path
                .file_stem()
                .unwrap_or(src_path.as_os_str())
                .to_string_lossy();
            let obj_ext = if is_msvc { "obj" } else { "o" };
            let obj_path = obj_dir.join(format!("{}.{}", stem, obj_ext));

            // Construct Arguments
            let mut args = Vec::new();

            // CCache injection
            if let Some(wrapper) = ccache_prefix {
                args.push(wrapper.to_string());
            }
            args.push(compiler.clone());

            if is_msvc {
                // MSVC Flags
                args.push("/nologo".to_string()); // Suppress copyright
                args.push("/c".to_string());
                args.push("/EHsc".to_string()); // Standard C++ exceptions
                args.push(src_path.to_string_lossy().to_string());
                args.push(format!("/Fo{}", obj_path.to_string_lossy()));
                args.push(get_std_flag_msvc(&config.package.edition));

                // Recursive Header Tracking for MSVC
                // /sourceDependencies <file> available in VS 2019+
                args.push("/sourceDependencies".to_string());
                args.push(format!("{}.json", obj_path.display()));
            } else {
                // GCC/Clang Flags
                args.push("-fdiagnostics-color=always".to_string());
                args.push("-c".to_string());
                args.push(src_path.to_string_lossy().to_string());
                args.push("-o".to_string());
                args.push(obj_path.to_string_lossy().to_string());
                args.push(get_std_flag_gcc(&config.package.edition));

                // Generate Dependency File
                args.push("-MMD".to_string());
                args.push("-MF".to_string());
                args.push(obj_path.with_extension("d").to_string_lossy().to_string());
            }

            if release {
                if is_msvc {
                    args.push("/O2".to_string());
                } else {
                    args.push("-O3".to_string());
                }
            } else if is_msvc {
                args.push("/Z7".to_string()); // Debug info
                args.push("/W4".to_string());
            } else {
                args.push("-g".to_string());
                args.push("-Wall".to_string());
            }

            if let Some(build_cfg) = &config.build
                && let Some(flags) = &build_cfg.cflags
            {
                for flag in flags {
                    // Translate MSVC-style flags for GCC/Clang
                    let translated = if !is_msvc && flag.starts_with("/D") {
                        format!("-D{}", &flag[2..])
                    } else if !is_msvc && flag.starts_with("/I") {
                        format!("-I{}", &flag[2..])
                    } else if is_msvc && flag.starts_with("-D") {
                        format!("/D{}", &flag[2..])
                    } else if is_msvc && flag.starts_with("-I") {
                        format!("/I{}", &flag[2..])
                    } else {
                        flag.clone()
                    };
                    args.push(translated);
                }
            }
            args.extend(common_flags.iter().cloned());
            args.extend(pch_args.iter().cloned());

            // Prepare JSON entry for Intellisense
            let entry = json!({
                "directory": current_dir_str,
                "command": args.join(" "),
                "file": src_path.to_string_lossy()
            });

            // Incremental Check
            let needs_compile = if !obj_path.exists() {
                true
            } else {
                check_dependencies(&obj_path, src_path).unwrap_or(true)
            };

            // Profiling Start
            let compile_start = Instant::now();
            if needs_compile {
                pb.set_message(format!("Compiling {}", stem));
                let mut cmd = Command::new(&args[0]);
                cmd.args(&args[1..]);

                // Apply toolchain environment variables (INCLUDE, LIB, etc.)
                if !toolchain_env.is_empty() {
                    cmd.envs(&toolchain_env);
                }

                let output = cmd.output().context("Failed to execute compiler")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

                    let error_msg = format!(
                        "Error compiling {}:\n{}{}",
                        src_path.display(),
                        stdout,
                        stderr
                    );
                    pb.println(format!("{} {}", "x".red(), error_msg));

                    // Educational Feedback
                    if let Some(suggestion) = super::feedback::FeedbackAnalyzer::analyze(&stderr) {
                        pb.println(format!(
                            "\n{} {}\n",
                            "💡 Suggestion:".bold().yellow(),
                            suggestion
                        ));
                    }

                    return Err(anyhow::anyhow!("Compilation failed"));
                } else {
                    // Print warnings if any (buffered)
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !stderr.is_empty() {
                        pb.println(format!(
                            "{} Warning in {}:\n{}",
                            "!".yellow(),
                            src_path.display(),
                            stderr
                        ));
                    }
                    // Some compilers print warnings to stdout too
                    if !stdout.is_empty() {
                        pb.println(format!(
                            "{} Output in {}:\n{}",
                            "!".cyan(),
                            src_path.display(),
                            stdout
                        ));
                    }
                }
            }

            // Profiling End
            if let Some(events) = &trace_events {
                let duration = compile_start.elapsed();
                let ts = compile_start
                    .checked_duration_since(build_start_time)
                    .unwrap_or_default()
                    .as_micros();
                let dur = duration.as_micros();
                // Capture thread ID (best effort)
                let tid = rayon::current_thread_index().unwrap_or(0);

                if let Ok(mut lock) = events.lock() {
                    lock.push(TraceEvent {
                        name: src_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        cat: "compilation".to_string(),
                        ph: "X".to_string(),
                        ts,
                        dur,
                        pid: 1,
                        tid,
                    });
                }
            }

            pb.inc(1);
            Ok((obj_path, entry))
        })
        .collect::<Result<Vec<_>>>()?; // Collects errors if any

    pb.finish_with_message("Compilation complete");

    // Profiling Dump
    if let Some(events) = trace_events
        && let Ok(locked) = events.lock()
    {
        let json = serde_json::to_string(&*locked)?;
        let trace_path = Path::new(".cx").join("build").join("build_trace.json");
        if let Some(parent) = trace_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&trace_path, json)?;
        println!(
            "   {} Build trace saved to {} (Chrome Tracing)",
            "📊".blue(),
            trace_path.display()
        );
    }

    // Unzip results separate object files and JSON entries
    let (object_files, json_entries): (Vec<PathBuf>, Vec<serde_json::Value>) =
        results.into_iter().unzip();

    // 6. Generate compile_commands.json in .cx/build/
    let json_str = serde_json::to_string_pretty(&json_entries)?;
    let compile_commands_path = Path::new(".cx").join("build").join("compile_commands.json");
    if let Some(parent) = compile_commands_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&compile_commands_path, json_str)?;

    // 7. Linking
    let mut needs_link = !output_bin.exists();
    if !needs_link {
        let bin_time = fs::metadata(&output_bin)?.modified()?;
        for obj in &object_files {
            if fs::metadata(obj)?.modified()? > bin_time {
                needs_link = true;
                break;
            }
        }
    }

    if needs_link {
        println!("   {} Linking...", "🔗".cyan());

        // Check if we have MSVC .lib files in dependencies (requires MSVC-compatible linker)
        let has_msvc_libs = dep_libs.iter().any(|lib| lib.ends_with(".lib"));
        let is_windows = cfg!(target_os = "windows");
        let is_mingw_clang = !is_msvc && is_windows && compiler.contains("clang");

        // Use clang-cl if we have MinGW clang but need to link MSVC libs
        let effective_compiler = if is_mingw_clang && has_msvc_libs {
            println!(
                "   {} Using clang-cl for MSVC library compatibility",
                "⚡".yellow()
            );
            "clang-cl".to_string()
        } else {
            compiler.clone()
        };
        let use_clang_cl = effective_compiler == "clang-cl";

        let mut cmd = Command::new(&effective_compiler);

        // Link Flags for LTO
        if lto {
            if is_msvc {
                cmd.arg("/LTCG");
            } else {
                cmd.arg("-flto");
            }
        }

        // Link Flags for Sanitizers
        if let Some(checks) = &sanitize
            && !is_msvc
        {
            cmd.arg(format!("-fsanitize={}", checks));
        }

        cmd.args(&object_files);

        // Add include paths for source files in dep_libs (e.g., GLAD's gl.c)
        // When .c/.cpp files are passed to the linker, MSVC compiles them on the fly
        // and needs include paths to find headers like <glad/gl.h>
        let has_source_files = dep_libs.iter().any(|lib| {
            let lower = lib.to_lowercase();
            lower.ends_with(".c")
                || lower.ends_with(".cpp")
                || lower.ends_with(".cc")
                || lower.ends_with(".cxx")
        });

        if has_source_files {
            for path in &include_paths {
                if is_msvc || use_clang_cl {
                    cmd.arg(format!("/I{}", path.display()));
                } else {
                    cmd.arg(format!("-I{}", path.display()));
                }
            }
            cmd.args(&extra_cflags);
        }

        if is_msvc || use_clang_cl {
            // Use to_string_lossy and quote the path to handle spaces and special chars
            let output_path = output_bin.to_string_lossy();
            cmd.arg(format!("/Fe:{}", output_path));
            cmd.arg(format!("/Fo:{}", obj_dir.to_string_lossy()));
        } else {
            cmd.arg("-o").arg(&output_bin);
        }

        for lib in &dep_libs {
            cmd.arg(lib);
        }

        // Extract library search paths from dep_libs for user-specified libs
        // This allows libs = ["glfw3"] to find glfw3.lib in dependency directories
        let mut lib_search_paths = std::collections::HashSet::new();
        for lib in &dep_libs {
            let lib_path = Path::new(lib);
            if lib_path
                .extension()
                .map(|e| e == "lib" || e == "a")
                .unwrap_or(false)
                && let Some(parent) = lib_path.parent()
            {
                lib_search_paths.insert(parent.to_path_buf());
            }
        }

        // For GCC/Clang, add -L flags before the libs
        if !is_msvc && !use_clang_cl {
            for search_path in &lib_search_paths {
                cmd.arg(format!("-L{}", search_path.display()));
            }
        }

        if let Some(build_cfg) = &config.build
            && let Some(libs) = &build_cfg.libs
        {
            for lib in libs {
                if is_msvc || use_clang_cl {
                    cmd.arg(format!("{}.lib", lib));
                } else {
                    cmd.arg(format!("-l{}", lib));
                }
            }
        }

        // For MSVC, pass /LIBPATH: flags via /link at the end
        // Also ensure dynamic CRT (/MD) for compatibility with prebuilt libs like GLFW
        if (is_msvc || use_clang_cl) && !lib_search_paths.is_empty() {
            cmd.arg("/MD"); // Use dynamic CRT to match prebuilt dependencies
            cmd.arg("/link");
            for search_path in &lib_search_paths {
                cmd.arg(format!("/LIBPATH:{}", search_path.display()));
            }
            // Add subsystem flag if specified (e.g., for SDL2 with SDL2main.lib)
            if let Some(build_cfg) = &config.build
                && let Some(subsystem) = &build_cfg.subsystem
            {
                let subsystem_flag = match subsystem.to_lowercase().as_str() {
                    "windows" => "/SUBSYSTEM:WINDOWS",
                    "console" => "/SUBSYSTEM:CONSOLE",
                    _ => "/SUBSYSTEM:CONSOLE",
                };
                cmd.arg(subsystem_flag);
            }
        }

        // Apply toolchain environment variables (LIB, LIBPATH, etc.)
        if !toolchain_env.is_empty() {
            cmd.envs(&toolchain_env);
        }

        let output = cmd.output()?;
        if !output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("{}", stderr);
            println!("{} Linking failed", "x".red());

            if let Some(suggestion) = super::feedback::FeedbackAnalyzer::analyze(&stderr) {
                println!("\n{} {}\n", "💡 Suggestion:".bold().yellow(), suggestion);
            }

            return Ok(false);
        }

        // 8. Post-build Script
        if let Some(scripts) = &config.scripts
            && let Some(post) = &scripts.post_build
            && let Err(e) = run_script(post, &current_dir)
        {
            println!("{} Post-build script failed: {}", "x".red(), e);
        }

        println!(
            "{} Build finished in {:.2?}",
            "✓".green(),
            start_time.elapsed()
        );
    } else {
        println!("{} Up to date", "⚡".green());
    }

    Ok(true)
}

// --- COMMAND: Build & Run ---
pub fn build_and_run(
    release: bool,
    verbose: bool,
    dry_run: bool,
    run_args: Vec<String>,
    script_path: Option<String>,
) -> Result<()> {
    // 1. Determine Configuration
    let config = if let Some(path_str) = &script_path {
        // SCENARIO 1: Explicit Script Mode (e.g. `cx run 1.cpp`)
        let path = Path::new(path_str);

        // Implicit src/ lookup: check "src/<file>" if <file> doesn't exist
        let final_path = if !path.exists() && Path::new("src").join(path).exists() {
            Path::new("src").join(path)
        } else {
            path.to_path_buf()
        };

        if !final_path.exists() {
            // Fallback: Check if it's just a name without extension?
            // For now, strict check.
            anyhow::bail!("Script file not found: {}", path_str);
        }

        if verbose {
            eprintln!("{} Script Mode: {}", "⚡".yellow(), final_path.display());
        }

        let file_stem = final_path.file_stem().unwrap_or_default().to_string_lossy();

        let has_cpp = final_path.extension().is_some_and(|ext| {
            let s = ext.to_string_lossy();
            s == "cpp" || s == "cc" || s == "cxx"
        });

        // Try to load cx.toml first - this preserves project settings (deps, flags, libs)
        // Only fall back to ephemeral config if no cx.toml exists

        match load_config() {
            Ok(mut project_cfg) => {
                if verbose {
                    eprintln!("{} Using cx.toml settings for script build", "📦".cyan());
                }
                // Override sources to build only this file, but keep everything else
                if let Some(build_cfg) = &mut project_cfg.build {
                    build_cfg.sources = Some(vec![final_path.to_string_lossy().to_string()]);
                    // Use file stem as binary name for script builds
                    build_cfg.bin = Some(file_stem.to_string());
                } else {
                    // No [build] section, create one with just sources
                    project_cfg.build = Some(crate::config::BuildConfig {
                        sources: Some(vec![final_path.to_string_lossy().to_string()]),
                        bin: Some(file_stem.to_string()),
                        ..Default::default()
                    });
                }
                project_cfg
            }
            Err(_) => {
                // No cx.toml - use ephemeral config (pure script mode)
                if verbose {
                    eprintln!("{} No cx.toml found, using ephemeral config", "⚡".yellow());
                }
                let mut ephemeral =
                    crate::config::create_ephemeral_config(&file_stem, &file_stem, "auto", has_cpp);
                if let Some(build_cfg) = &mut ephemeral.build {
                    build_cfg.sources = Some(vec![final_path.to_string_lossy().to_string()]);
                }
                ephemeral
            }
        }
    } else {
        // SCENARIO 2: Project Mode (cx.toml) OR Default Script
        match load_config() {
            Ok(c) => c,
            Err(_) => {
                // Check if the first arg is a potential script file
                let maybe_script = run_args.first().and_then(|arg| {
                    let p = Path::new(arg);
                    if p.exists()
                        && !p.is_dir()
                        && p.extension().is_some_and(|ext| {
                            let s = ext.to_string_lossy();
                            ["cpp", "cc", "cxx", "c"].contains(&s.as_ref())
                        })
                    {
                        Some(p.to_owned())
                    } else {
                        None
                    }
                });

                if let Some(script) = maybe_script {
                    if verbose {
                        eprintln!(
                            "{} Script Mode (from args): {}",
                            "⚡".yellow(),
                            script.display()
                        );
                    }
                    let stem = script.file_stem().unwrap_or_default().to_string_lossy();
                    let has_cpp = script.extension().is_some_and(|e| {
                        let s = e.to_string_lossy();
                        s == "cpp" || s == "cc" || s == "cxx"
                    });

                    let mut cfg = crate::config::create_ephemeral_config(
                        &stem, &stem, "auto", // Let build_project detect
                        has_cpp,
                    );
                    if let Some(build_cfg) = &mut cfg.build {
                        build_cfg.sources = Some(vec![script.to_string_lossy().to_string()]);
                    }
                    cfg
                } else {
                    // Heuristics for default script (main.cpp etc)
                    let candidates = ["main.cpp", "src/main.cpp", "main.c", "src/main.c"];

                    let found = candidates.iter().find(|p| Path::new(p).exists());

                    if let Some(p) = found {
                        eprintln!("{} No cx.toml found, running: {}", "⚡".yellow(), p);
                        let path = Path::new(p);
                        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                        let has_cpp = path.extension().is_some_and(|e| {
                            let s = e.to_string_lossy();
                            s == "cpp" || s == "cc" || s == "cxx"
                        });

                        let mut cfg =
                            crate::config::create_ephemeral_config(&stem, &stem, "auto", has_cpp);
                        if let Some(build_cfg) = &mut cfg.build {
                            build_cfg.sources = Some(vec![p.to_string()]);
                        }
                        cfg
                    } else {
                        anyhow::bail!(
                            "cx.toml not found, and no default source file (main.cpp/c) detected."
                        );
                    }
                }
            }
        }
    };

    // Filter run_args: If the first argument matches the single source file in config (Script Mode via 'cx run script'),
    // we should remove it so the script doesn't receive its own filename as an argument.
    let run_args = if let Some(build) = &config.build
        && let Some(sources) = &build.sources
        && sources.len() == 1
        && !run_args.is_empty()
    {
        // Simple string check is usually sufficient as we set sources from args[0]
        if sources[0] == run_args[0] || Path::new(&sources[0]) == Path::new(&run_args[0]) {
            run_args[1..].to_vec()
        } else {
            run_args
        }
    } else {
        run_args
    };

    let options = BuildOptions {
        release,
        verbose,
        dry_run,
        ..Default::default()
    };

    let success = build_project(&config, &options)?;
    if !success {
        return Ok(());
    }

    // In dry-run mode, don't actually run
    if dry_run {
        println!("\n{}", "Run:".bold());
        let profile = if release { "release" } else { "debug" };
        let bin_basename = if let Some(build_cfg) = &config.build {
            build_cfg.bin.clone().unwrap_or(config.package.name.clone())
        } else {
            config.package.name.clone()
        };
        let bin_name = if cfg!(target_os = "windows") {
            format!("{}.exe", bin_basename)
        } else {
            bin_basename
        };
        let bin_path = Path::new("build").join(profile).join(&bin_name);

        // If script mode and 'src/' lookup happened, path might be tricky for bin name logic?
        // Ephemeral config uses file stem as bin name, so it should be fine.

        let bin_short = bin_path
            .file_name()
            .unwrap_or(bin_path.as_os_str())
            .to_string_lossy();
        let args_str = if run_args.is_empty() {
            String::new()
        } else {
            format!(" {}", run_args.join(" "))
        };
        println!("  → {}{}", bin_short.cyan(), args_str);
        return Ok(());
    }

    let profile = if release { "release" } else { "debug" };
    let bin_basename = if let Some(build_cfg) = &config.build {
        build_cfg.bin.clone().unwrap_or(config.package.name.clone())
    } else {
        config.package.name.clone()
    };

    let bin_name = if cfg!(target_os = "windows") {
        format!("{}.exe", bin_basename)
    } else {
        bin_basename
    };

    let bin_path = Path::new(".cx").join("build").join(profile).join(bin_name);

    if !bin_path.exists() {
        anyhow::bail!("Binary not found at {}", bin_path.display());
    }

    if verbose {
        println!("{} Running: {}\n", "🚀".green(), bin_path.display());
    } else {
        println!("{} Running...\n", "▶".green());
    }

    let mut run_cmd = Command::new(bin_path);
    run_cmd.args(run_args);
    let status = run_cmd.status()?;

    if !status.success() {
        // Don't error out, just return ok as we ran the program and it failed on its own terms
        // unless we want to propagate exit code.
        // Typically build tools separate build error vs run error.
    }

    Ok(())
}
