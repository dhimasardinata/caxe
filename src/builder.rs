use crate::config::CxConfig;
use crate::deps;
use anyhow::{Context, Result};
use colored::*;
use notify::{Config, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::Duration;
use walkdir::WalkDir;

pub fn clean() -> Result<()> {
    if Path::new("build").exists() {
        fs::remove_dir_all("build").context("Failed to remove build directory")?;
        println!("{} Build directory cleaned", "✓".green());
    } else {
        println!("{} Nothing to clean", "!".yellow());
    }
    Ok(())
}

pub fn build_and_run(release: bool, run_args: &[String]) -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    let config_str = fs::read_to_string("cx.toml")?;
    let config: CxConfig = toml::from_str(&config_str).context("Failed to parse cx.toml")?;

    println!(
        "{} Project: {} ({})",
        "🚀".blue(),
        config.package.name.bold(),
        config.package.edition
    );

    let mut include_flags = Vec::new();
    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            include_flags = deps::fetch_dependencies(deps)?;
        }
    }

    let mut source_files = Vec::new();
    let mut has_cpp = false;
    let mut needs_recompile = false;

    let output_bin = if cfg!(target_os = "windows") {
        "build/main.exe"
    } else {
        "build/main"
    };
    let bin_path = Path::new(output_bin);

    let bin_modified = if bin_path.exists() {
        fs::metadata(bin_path).ok().and_then(|m| m.modified().ok())
    } else {
        None
    };

    if bin_modified.is_none() {
        needs_recompile = true;
    }

    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if ["cpp", "cc", "cxx", "c"].contains(&s.as_ref()) {
                if s != "c" {
                    has_cpp = true;
                }
                source_files.push(path.to_owned());

                if let Some(bin_time) = bin_modified {
                    if let Ok(src_time) = fs::metadata(path).and_then(|m| m.modified()) {
                        if src_time > bin_time {
                            needs_recompile = true;
                        }
                    }
                }
            }
        }
    }

    if source_files.is_empty() {
        println!("{} No source files found.", "!".yellow());
        return Ok(());
    }

    let compiler = if has_cpp { "clang++" } else { "clang" };

    if needs_recompile {
        fs::create_dir_all("build")?;
        let mut cmd = Command::new(compiler);
        cmd.args(&source_files);
        cmd.arg("-o").arg(output_bin);
        cmd.arg(format!("-std={}", config.package.edition));

        if release {
            cmd.arg("-O3");
            println!("   {} Compiling (Release Mode)...", "🔥".red());
        } else {
            cmd.arg("-g").arg("-Wall");
            println!("   {} Compiling (Debug Mode)...", "⚙".blue());
        }

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

        if let Some(build_cfg) = &config.build {
            if let Some(libs) = &build_cfg.libs {
                for lib in libs {
                    cmd.arg(format!("-l{}", lib));
                }
            }
        }

        let output = cmd.output();
        match output {
            Ok(out) => {
                if !out.status.success() {
                    println!("{}", String::from_utf8_lossy(&out.stderr));
                    println!("{} Build failed", "x".red());
                    return Ok(());
                }
            }
            Err(_) => {
                println!("{} Compiler '{}' not found.", "x".red(), compiler);
                return Ok(());
            }
        }
        println!("{} Build finished", "✓".green());
    } else {
        println!("{} Up to date", "⚡".green());
    }

    println!("{} Running...\n", "▶".green());
    let run_path = format!("./{}", output_bin);
    let mut run_cmd = Command::new(&run_path);
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
    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            include_flags = deps::fetch_dependencies(deps)?;
        }
    }

    println!("{} Running tests...", "🧪".magenta());
    fs::create_dir_all("build/tests")?;

    let mut total_tests = 0;
    let mut passed_tests = 0;

    for entry in WalkDir::new("tests").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path
            .extension()
            .map_or(false, |ext| ext == "cpp" || ext == "cc")
        {
            total_tests += 1;
            let test_name = path.file_stem().unwrap().to_string_lossy();
            let output_bin = format!("build/tests/{}", test_name);

            print!("   TEST {} ... ", test_name.bold());

            let compiler = "clang++";
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
