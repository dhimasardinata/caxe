//! Test runner for C/C++ unit tests.
//!
//! This module provides the `cx test` command which compiles and runs
//! test files from the `tests/` directory.
//!
//! ## Features
//!
//! - Auto-links project sources for testing internals
//! - Parallel test compilation
//! - Test filtering with `--filter`

use super::utils::{get_compiler, get_std_flag_gcc, get_std_flag_msvc, load_config};
use crate::config::CxConfig;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

pub fn run_tests(filter: Option<String>) -> Result<()> {
    // Load config or default
    let config = load_config().unwrap_or_else(|_| CxConfig {
        package: crate::config::PackageConfig {
            name: "test_runner".into(),
            version: "0.0.0".into(),
            edition: "c++20".into(),
        },
        ..Default::default()
    });

    let test_dir_str = config
        .test
        .as_ref()
        .and_then(|t| t.source_dir.clone())
        .unwrap_or_else(|| "tests".to_string());
    let test_dir = Path::new(&test_dir_str);

    if !test_dir.exists() {
        println!("{} No {}/ directory found.", "!".yellow(), test_dir_str);
        return Ok(());
    }

    let mut include_paths = Vec::new();
    let mut extra_cflags = Vec::new();
    let mut dep_libs = Vec::new();

    if let Some(deps) = &config.dependencies
        && !deps.is_empty()
    {
        let (paths, cflags, libs) = crate::deps::fetch_dependencies(deps)?;
        include_paths = paths;
        extra_cflags = cflags;
        dep_libs = libs;
    }

    println!("{} Running tests...", "🧪".magenta());
    if let Some(f) = &filter {
        println!("   Filter: {}", f.cyan());
    }
    fs::create_dir_all("build/tests")?;

    // Collect Project Object Files (excluding main)
    // We assume the project was built in 'debug' mode for tests
    let obj_dir = Path::new("build/debug/obj");
    let mut project_objs = Vec::new();
    if obj_dir.exists() {
        for entry in WalkDir::new(obj_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "o" || e == "obj") {
                // Exclude main.o / main.obj to avoid multiple entry points
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                if stem != "main" {
                    project_objs.push(path.to_path_buf());
                }
            }
        }
    } else {
        println!(
            "{} Warning: Project not built. Running tests without linking project sources.",
            "!".yellow()
        );
        println!("   Run 'cx build' first to link project code.");
    }

    let mut test_files = Vec::new();
    for entry in WalkDir::new(test_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        let is_cpp = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ["cpp", "cc", "cxx"].contains(&ext));
        let is_c = path.extension().is_some_and(|ext| ext == "c");

        if is_cpp || is_c {
            // Apply Filter
            if let Some(f) = &filter {
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                if !name.contains(f) {
                    continue;
                }
            }
            test_files.push((path, is_cpp));
        }
    }

    if test_files.is_empty() {
        println!("{} No tests found.", "!".yellow());
        return Ok(());
    }

    // Check for Single Binary Mode
    // If enabled, we compile ALL test sources into ONE executable (runner)
    let single_binary = config
        .test
        .as_ref()
        .and_then(|t| t.single_binary)
        .unwrap_or(false);

    if single_binary {
        println!("{} Building single test runner...", "🔨".cyan());
        let runner_name = "test_runner";
        let output_bin = format!("build/tests/{}", runner_name); // Linux/Mac

        let compiler = get_compiler(&config, true); // Assume C++ for tests generally
        let is_msvc = compiler.contains("cl.exe") || compiler == "cl";

        let mut cmd = Command::new(&compiler);
        let mut args = Vec::new();

        if is_msvc {
            args.push("/nologo".to_string());
            args.push("/EHsc".to_string());
            args.push(format!("/Fe{}", output_bin));
            args.push(get_std_flag_msvc(&config.package.edition));

            // Includes
            for p in &include_paths {
                args.push(format!("/I{}", p.display()));
            }
            args.push("/Isrc".to_string());

            // Sources
            for (path, _) in &test_files {
                args.push(path.to_string_lossy().to_string());
            }
            // Project Objects
            for obj in &project_objs {
                args.push(obj.to_string_lossy().to_string());
            }

            // Libs
            args.push("/link".to_string());
            for lib in &dep_libs {
                args.push(lib.clone());
            }
        } else {
            args.push(get_std_flag_gcc(&config.package.edition));
            args.push("-o".to_string());
            args.push(output_bin.clone());

            // Includes
            for p in &include_paths {
                args.push(format!("-I{}", p.display()));
            }
            args.push("-Isrc".to_string());

            // Sources
            for (path, _) in &test_files {
                args.push(path.to_string_lossy().to_string());
            }
            // Project Objects
            for obj in &project_objs {
                args.push(obj.to_string_lossy().to_string());
            }

            // Libs
            for lib in &dep_libs {
                args.push(lib.clone());
            }
            if let Some(cfg) = &config.build
                && let Some(libs) = &cfg.libs
            {
                for lib in libs {
                    args.push(format!("-l{}", lib));
                }
            }
        }

        cmd.args(&args);

        // Execute Compilation
        let start = std::time::Instant::now();
        let output = cmd.output()?;
        if !output.status.success() {
            println!("{} Test Runner Compilation Failed:", "x".red());
            println!("{}", String::from_utf8_lossy(&output.stdout));
            println!("{}", String::from_utf8_lossy(&output.stderr));
            return Ok(());
        }
        println!("   {} Compiled in {:.2?}s", "✓".green(), start.elapsed());

        // Run It
        println!("{} Running tests...", "🚀".cyan());
        let run_path = if cfg!(target_os = "windows") {
            format!("{}.exe", output_bin)
        } else {
            format!("./{}", output_bin)
        };

        let mut run_cmd = Command::new(&run_path);
        // Pass filter as argument if present (standard for Catch2/GTest/doctest)
        if let Some(f) = &filter {
            run_cmd.arg(f);
        }

        let status = run_cmd.status()?;
        if status.success() {
            println!("{}", "TESTS PASSED".green().bold());
        } else {
            println!("{}", "TESTS FAILED".red().bold());
        }
        return Ok(());
    }

    let pb = ProgressBar::new((test_files.len() * 2) as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.green/black} {pos:>3}/{len:3} [{elapsed_precise}] {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("●○·"),
    );

    // Phase 1: Parallel Compilation
    let compiled_results: Vec<(String, Option<String>)> = test_files
        .par_iter()
        .map(|(path, is_cpp)| {
            let test_name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let output_bin = format!("build/tests/{}", test_name);

            // Caching Check: Compare mtime of test source vs test binary
            // In a real system, we'd check dependencies too, but this is a start.
            let bin_path = if cfg!(target_os = "windows") {
                format!("{}.exe", output_bin)
            } else {
                output_bin.clone()
            };

            let skip_compile = if Path::new(&bin_path).exists() {
                let src_mtime = fs::metadata(path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let bin_mtime = fs::metadata(&bin_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                // If source is older than binary, we *might* skip.
                // Ideally we also check project object mtimes, but let's keep it simple for now or check one level deep.
                src_mtime < bin_mtime
            } else {
                false
            };

            if skip_compile {
                pb.inc(1); // Skip compile step
                return (test_name, Some(output_bin));
            }

            pb.set_message(format!("Compiling {}", test_name));

            let compiler = get_compiler(&config, *is_cpp);
            let is_msvc = compiler.contains("cl.exe") || compiler == "cl";
            let mut cmd = Command::new(&compiler);

            if is_msvc {
                cmd.arg("/nologo");
                cmd.arg("/EHsc");
                cmd.arg(path);
                cmd.arg(format!("/Fe{}", output_bin)); // Output exe name
                cmd.arg(get_std_flag_msvc(&config.package.edition));

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("/I{}", p.display()));
                }
                // Include "src" so tests can "#include <main.hpp>" easily
                cmd.arg("/Isrc");
            } else {
                cmd.arg(path);
                cmd.arg("-o").arg(&output_bin);
                cmd.arg(get_std_flag_gcc(&config.package.edition));

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("-I{}", p.display()));
                }
                cmd.arg("-Isrc");
            }

            cmd.args(&extra_cflags);

            if let Some(build_cfg) = &config.build
                && let Some(flags) = build_cfg.get_flags()
            {
                cmd.args(flags);
            }

            // Link Libs & Project Objects
            if is_msvc {
                cmd.arg("/link");
            }
            cmd.args(&dep_libs);

            // Link Project Objects
            for obj in &project_objs {
                cmd.arg(obj);
            }

            if let Some(build_cfg) = &config.build
                && let Some(libs) = &build_cfg.libs
            {
                for lib in libs {
                    if is_msvc {
                        cmd.arg(format!("{}.lib", lib));
                    } else {
                        cmd.arg(format!("-l{}", lib));
                    }
                }
            }

            let output = cmd.output();
            let success = match output {
                Ok(out) => {
                    if !out.status.success() {
                        pb.suspend(|| {
                            println!("{} COMPILE FAIL: {}", "x".red(), test_name.bold());
                            println!("{}", String::from_utf8_lossy(&out.stdout));
                            println!("{}", String::from_utf8_lossy(&out.stderr));
                        });
                        false
                    } else {
                        true
                    }
                }
                Err(e) => {
                    pb.suspend(|| {
                        println!("{} COMPILER ERROR: {} ({})", "x".red(), test_name.bold(), e);
                    });
                    false
                }
            };

            pb.inc(1);
            if success {
                (test_name, Some(output_bin))
            } else {
                (test_name, None)
            }
        })
        .collect();

    // Phase 2: Sequential Execution (Running Tests)
    let mut passed_tests = 0;
    let mut total_tests = 0;

    for (test_name, bin_path) in compiled_results {
        total_tests += 1;

        if let Some(output_bin) = bin_path {
            pb.set_message(format!("Running {}", test_name));

            let run_path = if cfg!(target_os = "windows") {
                format!("{}.exe", output_bin)
            } else {
                format!("./{}", output_bin)
            };

            let run_status = Command::new(&run_path).status();

            match run_status {
                Ok(status) => {
                    if status.success() {
                        pb.suspend(|| {
                            println!(
                                "   {} TEST {} ... {}",
                                "✓".green(),
                                test_name.bold(),
                                "PASS".green()
                            )
                        });
                        passed_tests += 1;
                    } else {
                        pb.suspend(|| {
                            println!(
                                "   {} TEST {} ... {}",
                                "x".red(),
                                test_name.bold(),
                                "FAIL".red()
                            )
                        });
                    }
                }
                Err(_) => {
                    pb.suspend(|| {
                        println!(
                            "   {} TEST {} ... {}",
                            "x".red(),
                            test_name.bold(),
                            "EXEC FAIL".red()
                        )
                    });
                }
            }
        }

        pb.inc(1);
    }

    pb.finish_and_clear();

    println!("\nTest Result: {}/{} passed.", passed_tests, total_tests);
    if total_tests > 0 && passed_tests == total_tests {
        println!("{}", "ALL TESTS PASSED ✨".green().bold());
    } else if total_tests > 0 {
        println!("{}", "SOME TESTS FAILED 💀".red().bold());
    }

    Ok(())
}
