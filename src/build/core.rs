use super::utils::{get_compiler, load_config, run_script};
use crate::config::CxConfig;
use crate::deps;
use anyhow::{Context, Result};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use walkdir::WalkDir;

// --- Helper: Check Dependencies (.d file) ---
fn check_dependencies(obj_path: &Path) -> Result<bool> {
    let d_path = obj_path.with_extension("d");
    if !d_path.exists() {
        return Ok(true); // No dependency file, force recompile
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
pub fn build_project(
    config: &CxConfig,
    release: bool,
    verbose: bool,
    dry_run: bool,
) -> Result<bool> {
    let start_time = Instant::now();
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
        let compiler_str = config
            .build
            .as_ref()
            .and_then(|b| b.compiler.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("auto");

        println!();
        if dry_run {
            println!("╭────────────────────────────────────────╮");
            println!("│  {} {:<30} │", "🔍".yellow(), "DRY RUN".bold().yellow());
        } else {
            println!("╭────────────────────────────────────────╮");
            println!("│  {} {:<30} │", "🔧".cyan(), "BUILD".bold().cyan());
        }
        println!("├────────────────────────────────────────┤");
        println!(
            "│  {:<10} {} v{:<19} │",
            "Package".dimmed(),
            config.package.name.cyan(),
            config.package.version
        );
        println!(
            "│  {:<10} {:<27} │",
            "Profile".dimmed(),
            if release {
                profile_str.green().to_string()
            } else {
                profile_str.yellow().to_string()
            }
        );
        println!("│  {:<10} {:<27} │", "Edition".dimmed(), edition_str);
        println!(
            "│  {:<10} {:<27} │",
            "Compiler".dimmed(),
            compiler_str.cyan()
        );
        println!("╰────────────────────────────────────────╯");
        println!();
    }

    // 1. Pre-build Script
    if let Some(scripts) = &config.scripts {
        if let Some(pre) = &scripts.pre_build {
            if verbose {
                println!("{} Running pre-build script: {}", "→".blue(), pre);
            }
            if let Err(e) = run_script(pre, &current_dir) {
                println!("{} Pre-build script failed: {}", "x".red(), e);
                return Ok(false);
            }
        }
    }

    // 2. Setup Directories
    let profile = if release { "release" } else { "debug" };
    let build_dir = Path::new("build").join(profile);
    let obj_dir = build_dir.join("obj");
    fs::create_dir_all(&obj_dir)?;

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

    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            let (paths, cflags, libs) = deps::fetch_dependencies(deps)?;
            include_paths = paths;
            extra_cflags = cflags;
            dep_libs = libs;
        }
    }

    // 4. Collect Source Files
    let mut source_files = Vec::new();
    let mut has_cpp = false;

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

    if source_files.is_empty() {
        println!("{} No source files found.", "!".yellow());
        return Ok(false);
    }

    // Get toolchain with environment variables
    let toolchain = super::utils::get_toolchain(config, has_cpp).ok();
    let compiler = if let Some(ref tc) = toolchain {
        tc.cxx_path.to_string_lossy().to_string()
    } else {
        get_compiler(config, has_cpp)
    };
    let is_msvc = compiler.contains("cl.exe") || compiler == "cl";
    let current_dir_str = current_dir.to_string_lossy().to_string();

    // Verbose: Show toolchain info
    if verbose {
        println!("{}", "Toolchain:".bold());
        println!("  Compiler: {}", compiler.cyan());
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

            let cmd = if is_msvc {
                format!(
                    "  → {} -c {} → {}",
                    short_compiler,
                    src_path.display(),
                    obj_path
                        .file_name()
                        .unwrap_or(obj_path.as_os_str())
                        .to_string_lossy()
                )
            } else {
                format!(
                    "  → {} -c {} → {}",
                    short_compiler,
                    src_path.display(),
                    obj_path
                        .file_name()
                        .unwrap_or(obj_path.as_os_str())
                        .to_string_lossy()
                )
            };
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

        // Modern footer
        println!();
        println!("╭────────────────────────────────────────╮");
        println!("│  {} {:<30} │", "✓".green(), "Dry run complete");
        println!("│  {:<38} │", "No commands were executed".dimmed());
        println!("╰────────────────────────────────────────╯");
        return Ok(true);
    }

    // 6. Parallel Compilation (Lock-Free Optimization)
    let spinner_style = ProgressStyle::default_spinner()
        .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("#>-");

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
            args.push(compiler.clone());

            if is_msvc {
                // MSVC Flags
                args.push("/nologo".to_string()); // Suppress copyright
                args.push("/c".to_string());
                args.push("/EHsc".to_string()); // Standard C++ exceptions
                args.push(src_path.to_string_lossy().to_string());
                args.push(format!("/Fo{}", obj_path.to_string_lossy()));
                args.push(format!("/std:{}", config.package.edition));

                // TODO: Recursive Header Tracking for MSVC (/sourceDependencies)
            } else {
                // GCC/Clang Flags
                args.push("-fdiagnostics-color=always".to_string());
                args.push("-c".to_string());
                args.push(src_path.to_string_lossy().to_string());
                args.push("-o".to_string());
                args.push(obj_path.to_string_lossy().to_string());
                args.push(format!("-std={}", config.package.edition));

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
            } else {
                if is_msvc {
                    args.push("/Z7".to_string()); // Debug info
                    args.push("/W4".to_string());
                } else {
                    args.push("-g".to_string());
                    args.push("-Wall".to_string());
                }
            }

            if let Some(build_cfg) = &config.build {
                if let Some(flags) = &build_cfg.cflags {
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
            }
            args.extend(common_flags.iter().cloned());

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
                match check_dependencies(&obj_path) {
                    Ok(needs) => needs,
                    Err(_) => true, // On error (e.g. read failure), safe to recompile
                }
            };

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
                    return Err(anyhow::anyhow!(error_msg));
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

            pb.inc(1);
            Ok((obj_path, entry))
        })
        .collect::<Result<Vec<_>>>()?; // Collects errors if any

    pb.finish_with_message("Compilation complete");

    // Unzip results separate object files and JSON entries
    let (object_files, json_entries): (Vec<PathBuf>, Vec<serde_json::Value>) =
        results.into_iter().unzip();

    // 6. Generate compile_commands.json
    let json_str = serde_json::to_string_pretty(&json_entries)?;
    fs::write("compile_commands.json", json_str)?;

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

        cmd.args(&object_files);

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

        if let Some(build_cfg) = &config.build {
            if let Some(libs) = &build_cfg.libs {
                for lib in libs {
                    if is_msvc || use_clang_cl {
                        cmd.arg(format!("{}.lib", lib));
                    } else {
                        cmd.arg(format!("-l{}", lib));
                    }
                }
            }
        }

        // Apply toolchain environment variables (LIB, LIBPATH, etc.)
        if !toolchain_env.is_empty() {
            cmd.envs(&toolchain_env);
        }

        let output = cmd.output()?;
        if !output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
            println!("{}", String::from_utf8_lossy(&output.stderr));
            println!("{} Linking failed", "x".red());
            return Ok(false);
        }

        // 8. Post-build Script
        if let Some(scripts) = &config.scripts {
            if let Some(post) = &scripts.post_build {
                if let Err(e) = run_script(post, &current_dir) {
                    println!("{} Post-build script failed: {}", "x".red(), e);
                }
            }
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
    run_args: &[String],
) -> Result<()> {
    // Load config once here
    let config = load_config()?;

    let success = build_project(&config, release, verbose, dry_run)?;
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

        // Footer (build_project already showed the main one)
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

    let bin_path = Path::new("build").join(profile).join(bin_name);

    println!("{} Running...\n", "▶".green());
    let mut run_cmd = Command::new(bin_path);
    run_cmd.args(run_args);
    let _ = run_cmd.status();

    Ok(())
}
