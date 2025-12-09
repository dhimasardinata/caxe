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

// --- CORE: Build Project ---
pub fn build_project(config: &CxConfig, release: bool) -> Result<bool> {
    let start_time = Instant::now();
    let current_dir = std::env::current_dir()?;

    // 1. Pre-build Script
    if let Some(scripts) = &config.scripts {
        if let Some(pre) = &scripts.pre_build {
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

    // 3. Fetch Dependencies
    let mut include_flags = Vec::new();
    let mut dep_libs = Vec::new();
    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            let (incs, libs) = deps::fetch_dependencies(deps)?;
            include_flags = incs;
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

    let compiler = get_compiler(config, has_cpp);
    let common_flags = include_flags.clone();
    let current_dir_str = current_dir.to_string_lossy().to_string();

    // 5. Parallel Compilation (Lock-Free Optimization)
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
            let stem = src_path.file_stem().unwrap().to_string_lossy();
            let obj_path = obj_dir.join(format!("{}.o", stem));

            // Construct Arguments
            let mut args = Vec::new();
            args.push(compiler.clone());

            // Force colors
            args.push("-fdiagnostics-color=always".to_string());

            args.push("-c".to_string());
            args.push(src_path.to_string_lossy().to_string());
            args.push("-o".to_string());
            args.push(obj_path.to_string_lossy().to_string());
            args.push(format!("-std={}", config.package.edition));

            if release {
                args.push("-O3".to_string());
            } else {
                args.push("-g".to_string());
                args.push("-Wall".to_string());
            }

            if let Some(build_cfg) = &config.build {
                if let Some(flags) = &build_cfg.cflags {
                    args.extend(flags.iter().cloned());
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
                let src_time = fs::metadata(src_path)?.modified()?;
                let obj_time = fs::metadata(&obj_path)?.modified()?;
                src_time > obj_time
            };

            if needs_compile {
                pb.set_message(format!("Compiling {}", stem));
                let mut cmd = Command::new(&args[0]);
                cmd.args(&args[1..]);
                let output = cmd.output().context("Failed to execute compiler")?;

                if !output.status.success() {
                    let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    pb.println(format!(
                        "{} Error compiling {}:\n{}",
                        "x".red(),
                        src_path.display(),
                        err_msg
                    ));
                    return Err(anyhow::anyhow!("Compilation failed"));
                } else {
                    // Print warnings if any (buffered)
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.is_empty() {
                        pb.println(format!(
                            "{} Warning in {}:\n{}",
                            "!".yellow(),
                            src_path.display(),
                            stderr
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
        let mut cmd = Command::new(&compiler);
        cmd.args(&object_files);
        cmd.arg("-o").arg(&output_bin);

        for lib in &dep_libs {
            cmd.arg(lib);
        }

        if let Some(build_cfg) = &config.build {
            if let Some(libs) = &build_cfg.libs {
                for lib in libs {
                    cmd.arg(format!("-l{}", lib));
                }
            }
        }

        let output = cmd.output()?;
        if !output.status.success() {
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
pub fn build_and_run(release: bool, run_args: &[String]) -> Result<()> {
    // Load config once here
    let config = load_config()?;

    let success = build_project(&config, release)?;
    if !success {
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
