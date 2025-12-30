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

use super::utils::{get_compiler, get_std_flag_gcc, get_std_flag_msvc, get_toolchain, load_config};
use crate::config::CxConfig;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
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
        let (paths, cflags, libs, _modules) = crate::deps::fetch_dependencies(deps)?;
        include_paths = paths;
        extra_cflags = cflags;
        dep_libs = libs;
    }

    println!("{} Running tests...", "🧪".magenta());
    if let Some(f) = &filter {
        println!("   Filter: {}", f.cyan());
    }
    let build_base = PathBuf::from(".cx/debug"); // TODO: Support release profile for tests
    let test_build_dir = build_base.join("tests");
    fs::create_dir_all(&test_build_dir)?;

    // Get toolchain for MSVC environment variables
    let toolchain = get_toolchain(&config, true).ok();
    let toolchain_env: std::collections::HashMap<String, String> = toolchain
        .as_ref()
        .map(|tc| tc.env_vars.clone())
        .unwrap_or_default();

    // Determine toolchain type for object file filtering
    let compiler = get_compiler(&config, true);
    let is_clang_cl = compiler.contains("clang-cl");
    let is_msvc = (compiler.contains("cl.exe") || compiler == "cl") && !is_clang_cl;
    let expected_obj_ext = if is_msvc { "obj" } else { "o" };

    // Collect module files from src/ (Moved up so we can filter them out of project objs)
    let mut module_files: Vec<PathBuf> = Vec::new();
    let src_dir = Path::new("src");
    if src_dir.exists() {
        for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ["cppm", "ixx", "mpp"].contains(&ext) {
                    module_files.push(path);
                }
            }
        }
    }
    module_files.sort(); // Ensure deterministic order

    // Collect Project Object Files (excluding main and modules)
    // We assume the project was built in 'debug' mode for tests
    let obj_dir = Path::new(".cx/debug/obj");
    let mut project_objs = Vec::new();

    // Helper to check if a path corresponds to a module object
    let is_module_obj = |path: &Path| -> bool {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        module_files.iter().any(|m| {
            let stem = m.file_stem().unwrap_or_default().to_string_lossy();
            file_name.starts_with(&*stem)
        })
    };

    if obj_dir.exists() {
        for entry in WalkDir::new(obj_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == expected_obj_ext) {
                // Exclude main.o / main.obj
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                if stem == "main" {
                    continue;
                }
                // Exclude module objects (they are added explicitly if needed)
                if is_module_obj(path) {
                    continue;
                }
                project_objs.push(path.to_path_buf());
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
        let test_name = config.package.name.clone(); // Use package name for the single test runner
        let output_bin = format!(".cx/tests/{}", test_name); // Linux/Mac

        let compiler = get_compiler(&config, true); // Assume C++ for tests generally
        let is_clang_cl = compiler.contains("clang-cl");
        let is_msvc = (compiler.contains("cl.exe") || compiler == "cl") && !is_clang_cl;

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

    // Compile modules first if any exist (sequential, like core.rs)
    let mut module_objs: Vec<PathBuf> = Vec::new();
    if !module_files.is_empty() {
        println!("{} Compiling project modules...", "📦".cyan());
        fs::create_dir_all(&obj_dir)?;

        let compiler = get_compiler(&config, true);
        let is_clang_cl = compiler.contains("clang-cl");
        let is_msvc = (compiler.contains("cl.exe") || compiler == "cl") && !is_clang_cl;

        for mod_path in &module_files {
            let stem = mod_path.file_stem().unwrap_or_default().to_string_lossy();
            let obj_ext = if is_msvc { "obj" } else { "o" };
            let obj_path = obj_dir.join(format!("{}.{}", stem, obj_ext));

            // Check if module needs recompilation
            let needs_compile = if !obj_path.exists() {
                true
            } else {
                let src_mtime = fs::metadata(mod_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let obj_mtime = fs::metadata(&obj_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                src_mtime > obj_mtime
            };

            if needs_compile {
                let mut cmd = Command::new(&compiler);

                if is_msvc {
                    cmd.args(["/nologo", "/c", "/EHsc", "/interface", "/TP", "/utf-8"]);
                    cmd.arg(mod_path);
                    cmd.arg(format!("/Fo{}", obj_path.display()));
                    cmd.arg(get_std_flag_msvc(&config.package.edition));
                    // MSVC needs include paths for module dependencies
                    for p in &include_paths {
                        cmd.arg(format!("/I{}", p.display()));
                    }
                    cmd.arg("/Iinclude");
                    cmd.arg("/Isrc");

                    // MSVC needs environment variables (INCLUDE, LIB, etc.)
                    if !toolchain_env.is_empty() {
                        cmd.envs(&toolchain_env);
                    }
                } else {
                    let is_clang = compiler.contains("clang");
                    let is_gcc =
                        (compiler.contains("gcc") || compiler.contains("g++")) && !is_clang;

                    if is_clang {
                        // Clang requires two-stage compilation:
                        // 1. Compile .cppm -> .pcm (BMI) with --precompile
                        // 2. Compile .pcm -> .o with -c
                        let pcm_path = obj_dir.join(format!("{}.pcm", stem));

                        // Stage 1: Compile to PCM
                        let mut cmd1 = Command::new(&compiler);
                        if is_clang_cl {
                            cmd1.arg(get_std_flag_msvc(&config.package.edition));
                        } else {
                            cmd1.arg(get_std_flag_gcc(&config.package.edition));
                        }
                        cmd1.arg("--precompile");
                        cmd1.arg(mod_path);
                        if is_clang_cl {
                            cmd1.arg(format!("/Fo{}", pcm_path.display()));
                        } else {
                            cmd1.arg("-o").arg(&pcm_path);
                        }
                        // Include paths
                        for p in &include_paths {
                            cmd1.arg(format!("-I{}", p.display()));
                        }
                        cmd1.arg("-Iinclude");
                        cmd1.arg("-Isrc");
                        if is_clang_cl {
                            cmd1.arg("/utf-8");
                        } else {
                            cmd1.arg("-finput-charset=UTF-8");
                            cmd1.arg("-fexec-charset=UTF-8");
                        }

                        let output1 = cmd1.output()?;
                        if !output1.status.success() {
                            println!("{} Module precompilation failed: {}", "x".red(), stem);
                            println!("{}", String::from_utf8_lossy(&output1.stdout));
                            println!("{}", String::from_utf8_lossy(&output1.stderr));
                            anyhow::bail!("Module compilation failed");
                        }

                        // Relocate PCM if clang-cl output it to CWD ignoring /Fo
                        if is_clang_cl && !pcm_path.exists() {
                            let cwd_pcm = Path::new(stem.as_ref()).with_extension("pcm");
                            if cwd_pcm.exists() {
                                fs::rename(&cwd_pcm, &pcm_path)?;
                            }
                        }

                        // Stage 2: Compile PCM to object
                        let mut cmd2 = Command::new(&compiler);
                        cmd2.arg("-c");
                        cmd2.arg(&pcm_path);
                        if is_clang_cl {
                            cmd2.arg(format!("/Fo{}", obj_path.display()));
                        } else {
                            cmd2.arg("-o").arg(&obj_path);
                        }

                        let output2 = cmd2.output()?;
                        if !output2.status.success() {
                            println!("{} Module object compilation failed: {}", "x".red(), stem);
                            println!("{}", String::from_utf8_lossy(&output2.stdout));
                            println!("{}", String::from_utf8_lossy(&output2.stderr));
                            anyhow::bail!("Module compilation failed");
                        }
                    } else {
                        // GCC with -fmodules-ts
                        let mut cmd = Command::new(&compiler);
                        if is_gcc {
                            cmd.arg("-fmodules-ts");
                        }
                        cmd.args(["-c"]);
                        cmd.arg(mod_path);
                        cmd.arg("-o").arg(&obj_path);
                        cmd.arg(get_std_flag_gcc(&config.package.edition));
                        // Include paths
                        for p in &include_paths {
                            cmd.arg(format!("-I{}", p.display()));
                        }
                        cmd.arg("-Iinclude");
                        cmd.arg("-Isrc");
                        cmd.arg("-finput-charset=UTF-8");
                        cmd.arg("-fexec-charset=UTF-8");

                        let output = cmd.output()?;
                        if !output.status.success() {
                            println!("{} Module compilation failed: {}", "x".red(), stem);
                            println!("{}", String::from_utf8_lossy(&output.stdout));
                            println!("{}", String::from_utf8_lossy(&output.stderr));
                            anyhow::bail!("Module compilation failed");
                        }
                    }
                }

                // MSVC executes here (cmd is only defined for MSVC case)
                if is_msvc {
                    let output = cmd.output()?;
                    if !output.status.success() {
                        println!("{} Module compilation failed: {}", "x".red(), stem);
                        println!("{}", String::from_utf8_lossy(&output.stdout));
                        println!("{}", String::from_utf8_lossy(&output.stderr));
                        anyhow::bail!("Module compilation failed");
                    }
                }
            }
            module_objs.push(obj_path);
        }
    }

    let use_modules = !module_objs.is_empty();

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
            let output_bin = if cfg!(target_os = "windows") {
                format!(".cx/debug/tests/{}.exe", test_name)
            } else {
                format!(".cx/debug/tests/{}", test_name)
            };

            // Caching Check: Compare mtime of test source vs test binary
            let bin_path = output_bin.clone();

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

            // Check if this test uses modules (has import statements)
            let test_uses_modules = use_modules && {
                if let Ok(content) = fs::read_to_string(path) {
                    content.lines().any(|line| {
                        let trimmed = line.trim();
                        trimmed.starts_with("import ") && trimmed.contains(';')
                    })
                } else {
                    false
                }
            };

            pb.set_message(format!("Compiling {}", test_name));

            let compiler = get_compiler(&config, *is_cpp);
            let is_clang_cl = compiler.contains("clang-cl");
            let is_msvc = (compiler.contains("cl.exe") || compiler == "cl") && !is_clang_cl;
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
                cmd.arg("/utf-8"); // UTF-8 source and execution charset
            } else {
                cmd.arg(path);
                if is_clang_cl {
                    cmd.arg(format!("/Fe{}", output_bin));
                } else {
                    cmd.arg("-o").arg(&output_bin);
                }
                if is_clang_cl {
                    cmd.arg(get_std_flag_msvc(&config.package.edition));
                } else {
                    cmd.arg(get_std_flag_gcc(&config.package.edition));
                }

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("-I{}", p.display()));
                }
                cmd.arg("-Isrc");
                if is_clang_cl {
                    cmd.arg("/utf-8");
                } else {
                    cmd.arg("-finput-charset=UTF-8");
                    cmd.arg("-fexec-charset=UTF-8");
                }
            }

            // Universal Module Support for Tests (only if test uses imports)
            if test_uses_modules {
                if is_msvc {
                    // MSVC: Point to directory containing .ifc files
                    cmd.arg(format!("/ifcSearchDir:{}", obj_dir.display()));
                } else if (compiler.contains("gcc") || compiler.contains("g++"))
                    && !compiler.contains("clang")
                {
                    cmd.arg("-fmodules-ts");
                } else if compiler.contains("clang") {
                    cmd.arg(format!("-fprebuilt-module-path={}", obj_dir.display()));
                }
            }

            cmd.args(&extra_cflags);

            // Add user flags with MSVC translation
            if let Some(build_cfg) = &config.build
                && let Some(flags) = build_cfg.get_flags()
            {
                for flag in flags {
                    // Skip GCC-only warning flags for MSVC
                    if is_msvc && (flag == "-Wall" || flag == "-Wextra" || flag.starts_with("-W")) {
                        continue;
                    }
                    // Translate -std= to /std: for MSVC
                    if is_msvc && flag.starts_with("-std=") {
                        // Skip - std flag is already set via get_std_flag_msvc
                        continue;
                    }
                    // Translate -I to /I for MSVC
                    if is_msvc && flag.starts_with("-I") {
                        cmd.arg(format!("/I{}", &flag[2..]));
                        continue;
                    }
                    // Translate -D to /D for MSVC
                    if is_msvc && flag.starts_with("-D") {
                        cmd.arg(format!("/D{}", &flag[2..]));
                        continue;
                    }
                    cmd.arg(flag);
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

            // Link Module Objects (only if test uses imports)
            if test_uses_modules {
                for obj in &module_objs {
                    cmd.arg(obj);
                }
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

            // MSVC needs environment variables (INCLUDE, LIB, etc.)
            if is_msvc && !toolchain_env.is_empty() {
                cmd.envs(&toolchain_env);
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
                output_bin.clone()
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
        anyhow::bail!("Tests failed: {}/{} passed", passed_tests, total_tests);
    }

    Ok(())
}
