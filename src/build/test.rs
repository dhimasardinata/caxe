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
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use walkdir::WalkDir;

#[derive(Clone)]
struct CompilerSpec {
    command: String,
    is_clang_cl: bool,
    is_msvc: bool,
}

impl CompilerSpec {
    fn detect(config: &CxConfig, has_cpp: bool) -> Self {
        let command = get_compiler(config, has_cpp);
        let is_clang_cl = command.contains("clang-cl");
        let is_msvc = (command.contains("cl.exe") || command == "cl") && !is_clang_cl;
        Self {
            command,
            is_clang_cl,
            is_msvc,
        }
    }
}

fn collect_project_sources(config: &CxConfig) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut regular = Vec::new();
    let mut modules = Vec::new();

    if let Some(build_cfg) = &config.build
        && let Some(explicit_sources) = &build_cfg.sources
    {
        for src in explicit_sources {
            let path = PathBuf::from(src);
            if !path.exists() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if ["cppm", "ixx", "mpp"].contains(&ext) {
                modules.push(path);
            } else if ["cpp", "cc", "cxx", "c"].contains(&ext) {
                regular.push(path);
            }
        }
    } else {
        let src_dir = Path::new("src");
        if src_dir.exists() {
            for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path().to_path_buf();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default();
                if ["cppm", "ixx", "mpp"].contains(&ext) {
                    modules.push(path);
                } else if ["cpp", "cc", "cxx", "c"].contains(&ext) {
                    regular.push(path);
                }
            }
        }
    }

    regular.sort();
    regular.dedup();
    modules.sort();
    modules.dedup();

    (regular, modules)
}

fn file_mtime_or_epoch(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn latest_mtime(paths: &[PathBuf]) -> SystemTime {
    paths
        .iter()
        .map(|p| file_mtime_or_epoch(p))
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn should_recompile_test_binary(
    test_source: &Path,
    output_binary: &Path,
    global_input_mtime: SystemTime,
) -> bool {
    if !output_binary.exists() {
        return true;
    }

    let src_mtime = file_mtime_or_epoch(test_source);
    let bin_mtime = file_mtime_or_epoch(output_binary);

    src_mtime > bin_mtime || global_input_mtime > bin_mtime
}

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
    let build_base = super::core::artifact_profile_dir(false);
    let test_build_dir = build_base.join("tests");
    fs::create_dir_all(&test_build_dir)?;

    // Get toolchain for MSVC environment variables
    let toolchain = get_toolchain(&config, true).ok();
    let toolchain_env: std::collections::HashMap<String, String> = toolchain
        .as_ref()
        .map(|tc| tc.env_vars.clone())
        .unwrap_or_default();

    // Detect compilers once (avoid repeated detection in parallel loop)
    let cpp_compiler = CompilerSpec::detect(&config, true);
    let c_compiler = CompilerSpec::detect(&config, false);
    let expected_obj_ext = if cpp_compiler.is_msvc { "obj" } else { "o" };
    let (project_sources, module_files) = collect_project_sources(&config);

    // Collect Project Object Files (excluding main and modules)
    // We assume the project was built in 'debug' mode for tests
    let obj_dir = build_base.join("obj");
    let mut project_objs = Vec::new();
    let module_obj_names: HashSet<String> = module_files
        .iter()
        .map(|src| super::core::object_file_name_for_source(src, expected_obj_ext))
        .collect();
    let main_obj_names: HashSet<String> = project_sources
        .iter()
        .filter(|src| src.file_stem().is_some_and(|stem| stem == "main"))
        .map(|src| super::core::object_file_name_for_source(src, expected_obj_ext))
        .collect();

    if obj_dir.exists() {
        for entry in WalkDir::new(&obj_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(expected_obj_ext) {
                let file_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                // Exclude main object files and module objects based on exact deterministic names.
                if main_obj_names.contains(file_name.as_str()) {
                    continue;
                }
                if module_obj_names.contains(file_name.as_str()) {
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
        let output_bin_path = if cfg!(target_os = "windows") {
            test_build_dir.join(format!("{}.exe", test_name))
        } else {
            test_build_dir.join(&test_name)
        };
        let output_bin = output_bin_path.to_string_lossy().to_string();

        let compiler = cpp_compiler.command.clone(); // Assume C++ for tests generally
        let is_msvc = cpp_compiler.is_msvc;

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
        let mut run_cmd = Command::new(&output_bin_path);
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
        fs::create_dir_all(obj_dir.as_path())?;

        let compiler = cpp_compiler.command.clone();
        let is_clang_cl = cpp_compiler.is_clang_cl;
        let is_msvc = cpp_compiler.is_msvc;

        for mod_path in &module_files {
            let stem = mod_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let obj_ext = if is_msvc { "obj" } else { "o" };
            let obj_path = super::core::object_file_path_for_source(&obj_dir, mod_path, obj_ext);

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
                            let cwd_pcm = Path::new(&stem).with_extension("pcm");
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

    let mut global_input_paths = project_objs.clone();
    global_input_paths.extend(module_objs.clone());
    for lib in &dep_libs {
        let lib_path = PathBuf::from(lib);
        if lib_path.exists() {
            global_input_paths.push(lib_path);
        }
    }
    if Path::new("cx.toml").exists() {
        global_input_paths.push(PathBuf::from("cx.toml"));
    }
    let global_input_mtime = latest_mtime(&global_input_paths);

    // Phase 1: Parallel Compilation
    let compiled_results: Vec<(String, Option<String>)> = test_files
        .par_iter()
        .map(|(path, is_cpp)| {
            let test_name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let output_bin_path = if cfg!(target_os = "windows") {
                test_build_dir.join(format!("{}.exe", test_name))
            } else {
                test_build_dir.join(&test_name)
            };
            let output_bin = output_bin_path.to_string_lossy().to_string();

            if !should_recompile_test_binary(path, &output_bin_path, global_input_mtime) {
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

            let compiler_spec = if *is_cpp {
                cpp_compiler.clone()
            } else {
                c_compiler.clone()
            };
            let compiler = compiler_spec.command.clone();
            let is_clang_cl = compiler_spec.is_clang_cl;
            let is_msvc = compiler_spec.is_msvc;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn should_recompile_when_source_is_newer() {
        let temp_dir = tempdir().unwrap();
        let dir = temp_dir.path();

        let src = dir.join("test.cpp");
        let bin = dir.join("test.bin");
        fs::write(&src, "int main(){}").unwrap();
        fs::write(&bin, "bin").unwrap();
        std::thread::sleep(Duration::from_secs(1));
        fs::write(&src, "int main(){return 0;}").unwrap();

        assert!(should_recompile_test_binary(
            &src,
            &bin,
            SystemTime::UNIX_EPOCH
        ));
    }

    #[test]
    fn should_recompile_when_global_input_is_newer() {
        let temp_dir = tempdir().unwrap();
        let dir = temp_dir.path();

        let src = dir.join("test.cpp");
        let bin = dir.join("test.bin");
        fs::write(&src, "int main(){}").unwrap();
        fs::write(&bin, "bin").unwrap();

        let future_global = file_mtime_or_epoch(&bin)
            .checked_add(Duration::from_secs(2))
            .unwrap_or(SystemTime::now());
        assert!(should_recompile_test_binary(&src, &bin, future_global));
    }
}
