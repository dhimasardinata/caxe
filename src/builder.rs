use crate::config::CxConfig;
use crate::deps;
use anyhow::{Context, Result};
use colored::*;
use notify::{Config, RecursiveMode, Watcher};
use rayon::prelude::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

fn get_compiler(config: &CxConfig, has_cpp: bool) -> String {
    if let Some(build) = &config.build {
        if let Some(compiler) = &build.compiler {
            return compiler.clone();
        }
    }

    if has_cpp {
        "clang++".to_string()
    } else {
        "clang".to_string()
    }
}

pub fn build_project(release: bool) -> Result<bool> {
    let start_time = Instant::now();

    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(false);
    }
    let config_str = fs::read_to_string("cx.toml")?;
    let config: CxConfig = toml::from_str(&config_str).context("Failed to parse cx.toml")?;

    let profile = if release { "release" } else { "debug" };
    let build_dir = Path::new("build").join(profile);
    let obj_dir = build_dir.join("obj");
    fs::create_dir_all(&obj_dir)?;

    let bin_name = if cfg!(target_os = "windows") {
        format!("{}.exe", config.package.name)
    } else {
        config.package.name.clone()
    };
    let output_bin = build_dir.join(&bin_name);

    let mut include_flags = Vec::new();
    let mut dep_libs = Vec::new();
    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            let (incs, libs) = deps::fetch_dependencies(deps)?;
            include_flags = incs;
            dep_libs = libs;
        }
    }

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

    let compiler = get_compiler(&config, has_cpp);
    let common_flags = include_flags.clone();

    let json_entries = std::sync::Mutex::new(Vec::new());
    let current_dir = std::env::current_dir()?.to_string_lossy().to_string();

    let object_files: Vec<PathBuf> = source_files
        .par_iter()
        .map(|src_path| -> Result<PathBuf> {
            let stem = src_path.file_stem().unwrap().to_string_lossy();
            let obj_path = obj_dir.join(format!("{}.o", stem));

            let mut args = Vec::new();
            args.push(compiler.clone());
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
                    for flag in flags {
                        args.push(flag.clone());
                    }
                }
            }
            for flag in &common_flags {
                args.push(flag.clone());
            }

            {
                let entry = json!({
                    "directory": current_dir,
                    "command": args.join(" "),
                    "file": src_path.to_string_lossy()
                });
                json_entries.lock().unwrap().push(entry);
            }

            let needs_compile = if !obj_path.exists() {
                true
            } else {
                let src_time = fs::metadata(src_path)?.modified()?;
                let obj_time = fs::metadata(&obj_path)?.modified()?;
                src_time > obj_time
            };

            if needs_compile {
                let mut cmd = Command::new(&args[0]);
                cmd.args(&args[1..]);

                let output = cmd.output().context("Failed to execute compiler")?;
                if !output.status.success() {
                    let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    println!(
                        "{} Error compiling {}:\n{}",
                        "x".red(),
                        src_path.display(),
                        err_msg
                    );
                    return Err(anyhow::anyhow!("Compilation failed"));
                }
            }
            Ok(obj_path)
        })
        .collect::<Result<Vec<_>>>()
        .map_err(|_| anyhow::anyhow!("One or more files failed to compile"))?;

    let entries = json_entries.into_inner().unwrap();
    let json_str = serde_json::to_string_pretty(&entries)?;
    fs::write("compile_commands.json", json_str)?;

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

pub fn build_and_run(release: bool, run_args: &[String]) -> Result<()> {
    let success = build_project(release)?;
    if !success {
        return Ok(());
    }

    let config_str = fs::read_to_string("cx.toml")?;
    let config: CxConfig = toml::from_str(&config_str)?;

    let profile = if release { "release" } else { "debug" };
    let bin_name = if cfg!(target_os = "windows") {
        format!("{}.exe", config.package.name)
    } else {
        config.package.name
    };

    let bin_path = Path::new("build").join(profile).join(bin_name);

    println!("{} Running...\n", "▶".green());
    let mut run_cmd = Command::new(bin_path);
    run_cmd.args(run_args);
    let _ = run_cmd.status();

    Ok(())
}

pub fn watch() -> Result<()> {
    println!("{} Watching for changes in src/...", "👀".cyan());
    let (tx, rx) = channel();
    let config = Config::default().with_poll_interval(Duration::from_secs(1));
    let mut watcher = notify::RecommendedWatcher::new(tx, config)?;

    watcher.watch(Path::new("src"), RecursiveMode::Recursive)?;

    run_and_clear();

    while let Ok(_) = rx.recv() {
        std::thread::sleep(Duration::from_millis(100));
        while let Ok(_) = rx.try_recv() {}
        run_and_clear();
    }
    Ok(())
}

fn run_and_clear() {
    print!("\x1B[2J\x1B[1;1H");
    println!("{} File changed. Rebuilding...", "🔄".yellow());
    if let Err(e) = build_and_run(false, &[]) {
        println!("{} Error: {}", "x".red(), e);
    }
}

pub fn clean() -> Result<()> {
    if Path::new("build").exists() {
        fs::remove_dir_all("build").context("Failed to remove build directory")?;
        println!("{} Build directory cleaned", "✓".green());
    } else {
        println!("{} Nothing to clean", "!".yellow());
    }
    Ok(())
}

pub fn run_tests() -> Result<()> {
    let test_dir = Path::new("tests");
    if !test_dir.exists() {
        println!("{} No tests/ directory found.", "!".yellow());
        return Ok(());
    }

    let config_str = fs::read_to_string("cx.toml").unwrap_or_default();
    let config: CxConfig = toml::from_str(&config_str).unwrap_or_else(|_| CxConfig {
        package: crate::config::PackageConfig {
            name: "test_runner".into(),
            version: "0.0.0".into(),
            edition: "c++20".into(),
        },
        ..Default::default()
    });

    let mut include_flags = Vec::new();
    let mut dep_libs = Vec::new();

    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            let (incs, libs) = deps::fetch_dependencies(deps)?;
            include_flags = incs;
            dep_libs = libs;
        }
    }

    println!("{} Running tests...", "🧪".magenta());
    fs::create_dir_all("build/tests")?;

    let mut total_tests = 0;
    let mut passed_tests = 0;

    for entry in WalkDir::new("tests").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_cpp = path
            .extension()
            .map_or(false, |ext| ext == "cpp" || ext == "cc" || ext == "cxx");
        let is_c = path.extension().map_or(false, |ext| ext == "c");
        if is_cpp || is_c {
            total_tests += 1;
            let test_name = path.file_stem().unwrap().to_string_lossy();
            let output_bin = format!("build/tests/{}", test_name);

            print!("   TEST {} ... ", test_name.bold());

            let compiler = get_compiler(&config, is_cpp);
            let mut cmd = Command::new(compiler);
            cmd.arg(path);
            cmd.arg("-o").arg(&output_bin);
            cmd.arg(format!("-std={}", config.package.edition));

            if let Some(build_cfg) = &config.build {
                if let Some(flags) = &build_cfg.cflags {
                    for flag in flags {
                        cmd.arg(flag);
                    }
                }
            }

            for flag in &include_flags {
                cmd.arg(flag);
            }

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

            let output = cmd.output();
            if output.is_err() || !output.as_ref().unwrap().status.success() {
                println!("{}", "COMPILE FAIL".red());
                if let Ok(out) = output {
                    println!("{}", String::from_utf8_lossy(&out.stderr));
                }
                continue;
            }

            let run_path = format!("./{}", output_bin);
            let run_status = Command::new(&run_path).status();

            match run_status {
                Ok(status) => {
                    if status.success() {
                        println!("{}", "PASS".green());
                        passed_tests += 1;
                    } else {
                        println!("{}", "FAIL".red());
                    }
                }
                Err(_) => println!("{}", "EXEC FAIL".red()),
            }
        }
    }

    println!("\nTest Result: {}/{} passed.", passed_tests, total_tests);
    if passed_tests == total_tests {
        println!("{}", "ALL TESTS PASSED ✨".green().bold());
    } else {
        println!("{}", "SOME TESTS FAILED 💀".red().bold());
    }

    Ok(())
}
