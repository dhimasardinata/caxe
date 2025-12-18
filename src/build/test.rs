use super::utils::{get_compiler, load_config};
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

    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            let (paths, cflags, libs) = crate::deps::fetch_dependencies(deps)?;
            include_paths = paths;
            extra_cflags = cflags;
            dep_libs = libs;
        }
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
            if path.extension().map_or(false, |e| e == "o" || e == "obj") {
                // Exclude main.o / main.obj to avoid multiple entry points
                let stem = path.file_stem().unwrap().to_string_lossy();
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
        let is_cpp = path.extension().map_or(false, |ext| {
            ["cpp", "cc", "cxx"].contains(&ext.to_str().unwrap())
        });
        let is_c = path.extension().map_or(false, |ext| ext == "c");

        if is_cpp || is_c {
            // Apply Filter
            if let Some(f) = &filter {
                let name = path.file_stem().unwrap().to_string_lossy();
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

    let pb = ProgressBar::new((test_files.len() * 2) as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Phase 1: Parallel Compilation
    let compiled_results: Vec<(String, Option<String>)> = test_files
        .par_iter()
        .map(|(path, is_cpp)| {
            let test_name = path.file_stem().unwrap().to_string_lossy().to_string();
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
                cmd.arg(format!("/std:{}", config.package.edition));

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("/I{}", p.display()));
                }
                // Include "src" so tests can "#include <main.hpp>" easily
                cmd.arg("/Isrc");
            } else {
                cmd.arg(path);
                cmd.arg("-o").arg(&output_bin);
                cmd.arg(format!("-std={}", config.package.edition));

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("-I{}", p.display()));
                }
                cmd.arg("-Isrc");
            }

            cmd.args(&extra_cflags);

            if let Some(build_cfg) = &config.build {
                if let Some(flags) = &build_cfg.cflags {
                    cmd.args(flags);
                }
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

            if let Some(build_cfg) = &config.build {
                if let Some(libs) = &build_cfg.libs {
                    for lib in libs {
                        if is_msvc {
                            cmd.arg(format!("{}.lib", lib));
                        } else {
                            cmd.arg(format!("-l{}", lib));
                        }
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
