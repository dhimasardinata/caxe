use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use git2::Repository;
use notify::{Config, RecursiveMode, Watcher};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::Duration;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "cx")]
#[command(about = "The modern C/C++ project manager", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    New {
        name: String,
        #[arg(long, default_value = "cpp")]
        lang: String,
    },
    Run {
        #[arg(long)]
        release: bool,
        #[arg(last = true)]
        args: Vec<String>,
    },
    Watch,
    Clean,
}

#[derive(Deserialize, Debug)]
struct CxConfig {
    package: PackageConfig,
    dependencies: Option<HashMap<String, String>>,
    build: Option<BuildConfig>,
}

#[derive(Deserialize, Debug)]
struct PackageConfig {
    name: String,
    #[allow(dead_code)]
    version: String,
    #[serde(default = "default_edition")]
    edition: String,
}

#[derive(Deserialize, Debug)]
struct BuildConfig {
    cflags: Option<Vec<String>>,
    libs: Option<Vec<String>>,
}

fn default_edition() -> String {
    "c++20".to_string()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::New { name, lang } => create_project(name, lang),
        Commands::Run { release, args } => run_project(*release, args),
        Commands::Watch => watch_project(),
        Commands::Clean => clean_project(),
    }
}

fn create_project(name: &str, lang: &str) -> Result<()> {
    let path = Path::new(name);
    if path.exists() {
        println!("{} Error: Directory '{}' already exists", "x".red(), name);
        return Ok(());
    }

    fs::create_dir_all(path.join("src"))?;

    let example_dep = if lang == "cpp" {
        "\n[dependencies]\n# json = \"https://github.com/nlohmann/json.git\""
    } else {
        ""
    };
    let config_content = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "{}"
{}
"#,
        name,
        if lang == "c" { "c17" } else { "c++20" },
        example_dep
    );

    fs::write(path.join("cx.toml"), config_content)?;

    let (filename, code) = if lang == "c" {
        (
            "main.c",
            "#include <stdio.h>\n\nint main() {\n    printf(\"Hello cx!\\n\");\n    return 0;\n}\n",
        )
    } else {
        (
            "main.cpp",
            "#include <iostream>\n\nint main() {\n    std::cout << \"Hello cx!\" << std::endl;\n    return 0;\n}\n",
        )
    };

    fs::write(path.join("src").join(filename), code)?;
    fs::write(path.join(".gitignore"), "/build\n")?;

    println!(
        "{} Created new {} project: {}",
        "✓".green(),
        lang.cyan(),
        name.bold()
    );
    Ok(())
}

fn clean_project() -> Result<()> {
    if Path::new("build").exists() {
        fs::remove_dir_all("build")?;
        println!("{} Build directory cleaned", "✓".green());
    }
    Ok(())
}

fn fetch_dependencies(deps: &HashMap<String, String>) -> Result<Vec<String>> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");
    fs::create_dir_all(&cache_dir)?;

    let mut include_paths = Vec::new();

    println!("{} Checking dependencies...", "📦".blue());

    for (name, url) in deps {
        let lib_path = cache_dir.join(name);

        if !lib_path.exists() {
            println!("   {} Downloading {} (Global Cache)...", "⬇".cyan(), name);
            println!("     URL: {}", url);

            match Repository::clone(url, &lib_path) {
                Ok(_) => println!("     Done."),
                Err(e) => {
                    println!("{} Failed to download {}: {}", "x".red(), name, e);
                    continue;
                }
            }
        } else {
            println!("   {} Using cached: {}", "⚡".green(), name);
        }

        include_paths.push(format!("-I{}", lib_path.display()));
        include_paths.push(format!("-I{}/include", lib_path.display()));
    }

    Ok(include_paths)
}

fn run_project(release: bool, run_args: &[String]) -> Result<()> {
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
            include_flags = fetch_dependencies(deps)?;
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
            cmd.arg("-g");
            cmd.arg("-Wall");
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
                println!(
                    "{} Compiler '{}' not found. Is it installed?",
                    "x".red(),
                    compiler
                );
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

    let run_status = run_cmd.status();

    if let Err(_) = run_status {
        println!("{} Failed to run binary.", "x".red());
    }

    Ok(())
}

fn watch_project() -> Result<()> {
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

    if let Err(e) = run_project(false, &[]) {
        println!("{} Error: {}", "x".red(), e);
    }
}
