//! # caxe CLI Entry Point
//!
//! This is the main executable for the `cx` command-line tool.
//! It parses CLI arguments using clap and routes commands to the appropriate handlers.
//!
//! ## Command Structure
//!
//! Commands are organized into categories:
//! - **Project**: `new`, `init`, `info`, `stats`
//! - **Build**: `build`, `run`, `clean`, `watch`, `test`
//! - **Dependencies**: `add`, `remove`, `update`, `vendor`, `tree`
//! - **Quality**: `fmt`, `check`, `doc`
//! - **Toolchain**: `toolchain`, `target`, `doctor`
//! - **Ecosystem**: `ci`, `docker`, `setup-ide`, `generate`

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use colored::*;
use inquire::{Select, Text};
use std::fs;
use std::path::{Path, PathBuf};

use caxe::build;
use caxe::cache;
use caxe::checker;
use caxe::ci;
use caxe::commands;
use caxe::deps;
use caxe::doc;
use caxe::docker;
use caxe::ide;
use caxe::import;
use caxe::package;
use caxe::registry;
use caxe::stats;
use caxe::templates;
use caxe::toolchain;
use caxe::tree;
use caxe::ui;
use caxe::upgrade;

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
    fn SetConsoleCP(wCodePageID: u32) -> i32;
}

#[cfg(windows)]
fn enable_windows_utf8_console() {
    unsafe {
        SetConsoleOutputCP(65001);
        SetConsoleCP(65001);
    }
}

#[cfg(not(windows))]
fn enable_windows_utf8_console() {}

#[derive(Parser)]
#[command(name = "cx")]
#[command(about = "The modern C/C++ project manager", version = env!("CARGO_PKG_VERSION"))]
#[command(long_about = None)]
#[command(propagate_version = true)]
#[command(infer_subcommands = false)]
#[command(allow_external_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project from a template
    New {
        /// Project name (optional, defaults to interactive)
        name: Option<String>,
        /// Language (cpp or c) [default: cpp]
        #[arg(long, default_value = "cpp")]
        lang: String,
        /// Template (console, web, raylib, sdl2, opengl) [default: console]
        #[arg(long, default_value = "console")]
        template: String,
    },
    /// Compile the current project
    Build {
        /// Build artifacts in release mode, with optimizations
        #[arg(long)]
        release: bool,
        /// Show detailed build commands and decisions
        #[arg(short, long)]
        verbose: bool,
        /// Show what would be executed without running
        #[arg(long)]
        dry_run: bool,
        /// Generate build trace (Chrome Tracing format)
        #[arg(long)]
        trace: bool,
        /// Compile to WebAssembly (requires Emscripten)
        #[arg(long)]
        wasm: bool,
        /// Enable Link Time Optimization
        #[arg(long)]
        lto: bool,
        /// Enable Sanitizers (address, thread, undefined, leak)
        #[arg(long)]
        sanitize: Option<String>,
        /// Build as Arduino project (uses arduino-cli)
        #[arg(long)]
        arduino: bool,
        /// Use a named profile (e.g., --profile esp32)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Compile and run the output binary
    Run {
        /// Build artifacts in release mode, with optimizations
        #[arg(long)]
        release: bool,
        /// Show detailed build commands and decisions
        #[arg(short, long)]
        verbose: bool,
        /// Show what would be executed without running
        #[arg(long)]
        dry_run: bool,
        /// Arguments passed to the target program
        #[arg(num_args = 0.., allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Add a dependency to the project
    Add {
        /// Library name or URL
        lib: String,
        /// Specific git tag
        #[arg(long)]
        tag: Option<String>,
        /// Specific git branch
        #[arg(long)]
        branch: Option<String>,
        /// Specific git revision
        #[arg(long)]
        rev: Option<String>,
    },
    /// Manage the dependency lockfile
    Lock {
        /// Update the lockfile to the latest compatible versions
        #[arg(long)]
        update: bool,
        /// Verify that the lockfile is in sync with cx.toml
        #[arg(long)]
        check: bool,
    },
    /// Synchronize dependencies with lockfile
    Sync,
    /// Package the application for distribution
    Package {
        /// Output filename (default: <project_name>-v<version>.zip)
        #[arg(long, short)]
        output: Option<String>,
        /// Build release before packaging (default: true)
        #[arg(long, default_value_t = true)]
        release: bool,
    },
    /// Remove a dependency from cx.toml
    Remove {
        /// Library name to remove
        lib: String,
    },
    /// Watch source files and trigger build/test on change
    Watch {
        /// Watch tests instead of just building
        #[arg(long)]
        test: bool,
    },
    /// Clean build artifacts and cache
    Clean {
        /// Clean global dependency cache
        #[arg(long)]
        cache: bool,
        /// Clean everything (build/ dir, docs, artifacts)
        #[arg(long)]
        all: bool,
        /// Remove unused dependencies from global cache
        #[arg(long)]
        unused: bool,
    },
    /// Run unit tests
    Test {
        /// Filter tests by name
        #[arg(long)]
        filter: Option<String>,
    },
    /// Show system and project setup info
    Info,
    /// Format code using clang-format
    Fmt {
        /// Check formatting without modifying files (CI mode)
        #[arg(long)]
        check: bool,
    },
    /// Generate documentation using Doxygen
    Doc,
    /// Static analysis using clang-tidy / cppcheck
    Check,
    /// Update dependencies to latest versions
    Update,
    /// Upgrade caxe itself (if installed via cargo)
    Upgrade,
    /// Search the registry for libraries
    Search {
        /// Query string
        query: String,
    },
    /// Initialize a new cx.toml in existing directory
    Init,
    /// Manage the global dependency cache
    Cache {
        #[command(subcommand)]
        op: CacheOp,
    },
    /// Generate shell completion scripts
    Completion { shell: Shell },
    /// Manage toolchain selection
    Toolchain {
        #[command(subcommand)]
        op: Option<ToolchainOp>,
    },
    /// Manage frameworks (integrated + dependency-alias entries)
    Framework {
        #[command(subcommand)]
        op: Option<FrameworkOp>,
    },
    /// Diagnose system and project issues
    Doctor,
    /// Vendor dependencies into local directory
    Vendor,
    /// Generate CI/CD workflow
    CI,
    /// Generate Dockerfile
    Docker,
    /// Generate IDE configuration (VSCode)
    SetupIde,
    /// Visualize dependency tree
    Tree,
    /// Show project statistics
    Stats,
    /// Manage cross-compilation targets (mutation subcommands are deferred in v0.3.x)
    Target {
        #[command(subcommand)]
        op: Option<TargetOp>,
    },
    /// Generate build system files (CMake, Ninja)
    Generate {
        #[command(subcommand)]
        format: GenerateFormat,
    },
    /// Upload Arduino sketch to board
    Upload {
        /// Serial port (e.g., COM3, /dev/ttyUSB0)
        #[arg(short, long)]
        port: Option<String>,
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Run a C/C++ file directly (Script Mode)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
enum CacheOp {
    /// Clean the cache
    Clean,
    /// List cached items
    Ls,
    /// Print cache directory path
    Path,
}

#[derive(Subcommand)]
enum ToolchainOp {
    /// List all available toolchains
    List,
    /// Interactively select a toolchain
    Select,
    /// Clear cached toolchain selection
    Clear,
    /// Install a portable toolchain (e.g., mingw)
    Install {
        /// Name of the toolchain to install (interactive if omitted)
        name: Option<String>,
    },
    /// Update/refresh toolchain cache (re-detect available toolchains)
    Update,
}

#[derive(Subcommand)]
enum TargetOp {
    /// List all available targets
    List,
    /// Add a target to the project (deferred in v0.3.x; use profiles)
    Add {
        /// Target name (windows-x64, linux-x64, macos-x64, wasm32, esp32)
        name: String,
    },
    /// Remove a target from the project (deferred in v0.3.x; use profiles)
    Remove {
        /// Target name
        name: String,
    },
    /// Set the default target (deferred in v0.3.x; use profiles)
    Default {
        /// Target name
        name: String,
    },
}

#[derive(Subcommand)]
enum GenerateFormat {
    /// Generate CMakeLists.txt
    Cmake,
    /// Generate build.ninja
    Ninja,
    /// Generate compile_commands.json (for IDE integration)
    CompileCommands,
}

#[derive(Subcommand)]
enum FrameworkOp {
    /// List all available frameworks
    List,
    /// Interactively select a framework
    Select,
    /// Add a specific framework to the project (integrated entries only)
    Add {
        /// Framework name (daxe, fmt, json, catch2, spdlog)
        name: String,
    },
    /// Remove framework from project
    Remove {
        /// Framework name
        name: String,
    },
    /// Show framework info
    Info {
        /// Framework name
        name: String,
    },
}

fn main() -> Result<()> {
    enable_windows_utf8_console();

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::New {
            name,
            lang,
            template,
        }) => create_project(name, lang, template),

        Some(Commands::Search { query }) => {
            let results = registry::search(query);
            if results.is_empty() {
                println!("{} No results found for '{}'", "x".red(), query);
            } else {
                let mut table = ui::Table::new(&["Name", "Type/Url"]);
                for (name, url) in results {
                    table.add_row(vec![name.bold().green().to_string(), url]);
                }
                table.print();
            }
            Ok(())
        }

        Some(Commands::Lock { update, check }) => {
            commands::doctor::handle_lock(*update, *check);
            Ok(())
        }

        Some(Commands::Sync) => {
            commands::doctor::handle_sync();
            Ok(())
        }

        Some(Commands::Package { output, release }) => {
            package::package_project(output.clone(), *release)
        }

        Some(Commands::Build {
            release,
            verbose,
            dry_run,
            trace,
            wasm,
            lto,
            sanitize,
            arduino,
            profile,
        }) => {
            // Auto-detect Arduino projects: check for .ino files or [arduino] config
            let has_ino_files = std::fs::read_dir(".")
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .any(|e| e.path().extension().is_some_and(|ext| ext == "ino"))
                })
                .unwrap_or(false);

            let has_arduino_config = build::load_config()
                .map(|c| c.arduino.is_some())
                .unwrap_or(false);

            // Handle Arduino builds (explicit flag OR auto-detected)
            if *arduino || has_ino_files || has_arduino_config {
                return build::arduino::build_arduino(*verbose);
            }

            let config = build::load_config()?;
            let options = build::BuildOptions {
                release: *release,
                verbose: *verbose,
                dry_run: *dry_run,
                enable_profile: *trace,
                wasm: *wasm,
                lto: *lto,
                sanitize: sanitize.clone(),
                profile: profile.clone(),
            };

            // Workspace Support
            if let Some(ws) = &config.workspace {
                println!(
                    "{} Building Workspace ({} members)...",
                    "🚀".cyan(),
                    ws.members.len()
                );
                let root_dir = std::env::current_dir()?;

                for member in &ws.members {
                    let member_path = root_dir.join(member);
                    if !member_path.exists() {
                        println!("{} Member '{}' not found", "x".red(), member);
                        continue;
                    }

                    println!("\n{} Building member: {}", "📦".blue(), member);
                    std::env::set_current_dir(&member_path)?;

                    // Reload config for member
                    match build::load_config() {
                        Ok(member_config) => {
                            if let Err(e) = build::build_project(&member_config, &options) {
                                println!("{} Build failed for {}: {}", "x".red(), member, e);
                                // Continue or exit? Usually fail fast?
                                std::env::set_current_dir(&root_dir)?;
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            println!("{} Failed to load config for {}: {}", "x".red(), member, e);
                            std::env::set_current_dir(&root_dir)?;
                            std::process::exit(1);
                        }
                    }
                }
                // Restore root (though we are done/exiting)
                std::env::set_current_dir(&root_dir)?;

                // Also build root if it has sources?
                // Logic: If [package] exists (mandatory currently) and has sources, build it?
                // Usually workspace root is just a container.
                // We'll check if root has `src` dir or explicit sources.
                // If not, we skip root build silently.
                // But build_project checks sources.
                // Let's just try to build root if user explicitly asks?
                // Or maybe the workspace config implies *only* members?
                // Let's assume root is strictly a workspace manager if [workspace] present, UNLESS it has src/main.cpp?
                // Safe bet: Don't auto-build root if it's a workspace, unless it looks like a project.
                // But `config` is loaded.
                // We can't easily skip it without modifying `build_project`.
                // Let's just finish here. The loop built the members.
                Ok(())
            } else {
                match build::build_project(&config, &options) {
                    Ok(true) => Ok(()),
                    Ok(false) => std::process::exit(1),
                    Err(e) => Err(e),
                }
            }
        }

        Some(Commands::Run {
            release,
            verbose,
            dry_run,
            args,
        }) => {
            // Detect script mode: if first arg looks like a source file, use it as script_path
            let (script_path, run_args) = if !args.is_empty() {
                let first_arg = &args[0];
                let path = Path::new(first_arg);
                let is_source = path.extension().is_some_and(|ext| {
                    let s = ext.to_string_lossy().to_lowercase();
                    ["cpp", "cc", "cxx", "c", "cppm", "ixx", "mpp"].contains(&s.as_str())
                });
                if is_source {
                    (Some(first_arg.clone()), args[1..].to_vec())
                } else {
                    (None, args.clone())
                }
            } else {
                (None, args.clone())
            };
            build::build_and_run(*release, *verbose, *dry_run, run_args, script_path)
        }

        Some(Commands::Watch { test }) => build::watch(*test),
        Some(Commands::Clean { cache, all, unused }) => build::clean(*cache, *all, *unused),
        Some(Commands::Test { filter }) => build::run_tests(filter.clone()),
        Some(Commands::Add {
            lib,
            tag,
            branch,
            rev,
        }) => deps::add_dependency(lib, tag.clone(), branch.clone(), rev.clone()),
        Some(Commands::Remove { lib }) => deps::remove_dependency(lib),
        Some(Commands::Info) => print_info(),
        Some(Commands::Fmt { check }) => checker::format_code(*check),
        Some(Commands::Doc) => doc::generate_docs(),
        Some(Commands::Check) => checker::check_code(),
        Some(Commands::Update) => deps::update_dependencies(),
        Some(Commands::Upgrade) => upgrade::check_and_upgrade(),
        Some(Commands::Init) => init_project(),
        Some(Commands::Cache { op }) => match op {
            CacheOp::Clean => cache::clean(),
            CacheOp::Ls => cache::list(),
            CacheOp::Path => cache::print_path(),
        },
        Some(Commands::Completion { shell }) => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(*shell, &mut cmd, bin_name, &mut std::io::stdout());
            Ok(())
        }
        Some(Commands::Toolchain { op }) => {
            let local_op = op.as_ref().map(|o| match o {
                ToolchainOp::List => commands::toolchain::ToolchainOp::List,
                ToolchainOp::Select => commands::toolchain::ToolchainOp::Select,
                ToolchainOp::Clear => commands::toolchain::ToolchainOp::Clear,
                ToolchainOp::Install { name } => {
                    commands::toolchain::ToolchainOp::Install { name: name.clone() }
                }
                ToolchainOp::Update => commands::toolchain::ToolchainOp::Update,
            });
            commands::toolchain::handle_toolchain_command(&local_op)
        }
        Some(Commands::Framework { op }) => {
            let local_op = op.as_ref().map(|o| match o {
                FrameworkOp::List => commands::framework::FrameworkOp::List,
                FrameworkOp::Select => commands::framework::FrameworkOp::Select,
                FrameworkOp::Add { name } => {
                    commands::framework::FrameworkOp::Add { name: name.clone() }
                }
                FrameworkOp::Remove { name } => {
                    commands::framework::FrameworkOp::Remove { name: name.clone() }
                }
                FrameworkOp::Info { name } => {
                    commands::framework::FrameworkOp::Info { name: name.clone() }
                }
            });
            commands::framework::handle_framework_command(&local_op)
        }
        Some(Commands::Doctor) => commands::doctor::run_doctor(),
        Some(Commands::Vendor) => deps::vendor_dependencies(),
        Some(Commands::CI) => ci::generate_ci_config(),
        Some(Commands::Docker) => docker::generate_docker_config(),
        Some(Commands::SetupIde) => ide::generate_ide_config(),
        Some(Commands::Tree) => tree::print_tree(),
        Some(Commands::Stats) => stats::print_stats(),
        Some(Commands::Target { op }) => {
            let local_op = op.as_ref().map(|o| match o {
                TargetOp::List => commands::target::TargetOp::List,
                TargetOp::Add { name } => commands::target::TargetOp::Add { name: name.clone() },
                TargetOp::Remove { name } => {
                    commands::target::TargetOp::Remove { name: name.clone() }
                }
                TargetOp::Default { name } => {
                    commands::target::TargetOp::Default { name: name.clone() }
                }
            });
            commands::target::handle_target_command(&local_op)
        }
        Some(Commands::Generate { format }) => {
            let local_format = match format {
                GenerateFormat::Cmake => commands::generate::GenerateFormat::Cmake,
                GenerateFormat::Ninja => commands::generate::GenerateFormat::Ninja,
                GenerateFormat::CompileCommands => {
                    commands::generate::GenerateFormat::CompileCommands
                }
            };
            commands::generate::handle_generate_command(&local_format)
        }
        Some(Commands::Upload { port, verbose }) => {
            build::arduino::upload_arduino(port.clone(), *verbose)
        }
        Some(Commands::External(args)) => {
            if args.is_empty() {
                anyhow::bail!("No command provided");
            }
            // Treat args[0] as script path, args[1..] as run args
            let script_path = Some(args[0].clone());
            let run_args = args[1..].to_vec();

            // Script mode defaults: release=false, verbose=false, dry_run=false
            build::build_and_run(false, false, false, run_args, script_path)
        }
        None => {
            print_splash();
            Ok(())
        }
    }
}

fn print_splash() {
    println!();
    println!("   {}", " ██████  █████  ██   ██ ███████ ".cyan());
    println!("   {}", "██      ██   ██  ██ ██  ██      ".cyan());
    println!("   {}", "██      ███████   ███   █████   ".cyan());
    println!("   {}", "██      ██   ██  ██ ██  ██      ".cyan());
    println!("   {}", " ██████ ██   ██ ██   ██ ███████ ".cyan());
    println!();
    println!(
        "   {}",
        "The Modern C/C++ Project Manager".dimmed().italic()
    );
    println!("   {}", format!("v{}", env!("CARGO_PKG_VERSION")).green());
    println!();

    // Command Dashboard
    let mut table = ui::Table::new(&["Category", "Commands"]);

    table.add_row(vec![
        "Start".bold().green().to_string(),
        format!("{}, {}", "new".cyan(), "init".cyan()),
    ]);
    table.add_row(vec![
        "Build".bold().yellow().to_string(),
        format!(
            "{}, {}, {}, {}",
            "build".cyan(),
            "run".cyan(),
            "test".cyan(),
            "watch".cyan()
        ),
    ]);
    table.add_row(vec![
        "Deps".bold().blue().to_string(),
        format!(
            "{}, {}, {}, {}, {}",
            "add".cyan(),
            "remove".cyan(),
            "search".cyan(),
            "vendor".cyan(),
            "tree".cyan()
        ),
    ]);
    table.add_row(vec![
        "Tools".bold().magenta().to_string(),
        format!(
            "{}, {}, {}, {}, {}",
            "fmt".cyan(),
            "doc".cyan(),
            "check".cyan(),
            "docker".cyan(),
            "stats".cyan()
        ),
    ]);
    table.add_row(vec![
        "Config".bold().white().to_string(),
        format!(
            "{}, {}, {}",
            "toolchain".cyan(),
            "setup-ide".cyan(),
            "config".dimmed()
        ), // config is planned/implicit
    ]);

    table.print();
    println!();
    println!("   Run {} for detailed usage.", "cx --help".white().bold());
    println!();
}

fn init_project() -> Result<()> {
    // 1. Check existing
    if Path::new("cx.toml").exists() {
        println!(
            "{} Error: Project already initialized (cx.toml exists).",
            "x".red()
        );
        return Ok(());
    }

    // 2. Check for existing project structure for Import
    let current_dir = std::env::current_dir()?;
    let has_cmake = current_dir.join("CMakeLists.txt").exists();
    let has_src = current_dir.join("src").exists();
    let has_sources = std::fs::read_dir(&current_dir)
        .map(|read_dir| {
            read_dir.filter_map(|e| e.ok()).any(|e| {
                let p = e.path();
                if let Some(ext) = p.extension() {
                    let s = ext.to_string_lossy();
                    s == "cpp" || s == "c" || s == "cc"
                } else {
                    false
                }
            })
        })
        .unwrap_or(false);

    if has_cmake || has_src || has_sources {
        println!("{}", "Existing C/C++ project detected!".bold().yellow());
        let confirm = inquire::Confirm::new("Generate cx.toml automatically from this project?")
            .with_default(true)
            .prompt()?;

        if confirm && let Some(config) = import::scan_project(&current_dir)? {
            let toml_str = toml::to_string(&config)?;
            fs::write("cx.toml", toml_str)?;
            println!(
                "{} Imported project successfully. Run {} to build.",
                "✓".green(),
                "cx run".bold().white()
            );
            return Ok(());
        }
    }

    // 3. Interactive Inputs
    let current_dir = std::env::current_dir()?;
    let dir_name = current_dir
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("unknown"))
        .to_string_lossy();

    let name = Text::new("Project name?")
        .with_default(&dir_name)
        .prompt()?;

    let lang = Select::new("Language?", vec!["cpp", "c"]).prompt()?;
    let template = Select::new(
        "Template?",
        vec!["console", "arduino", "web", "raylib", "sdl2", "opengl"],
    )
    .prompt()?;

    let (toml_content, main_code) = templates::get_template(&name, lang, template);

    fs::write("cx.toml", toml_content)?;

    // Create src if generic template (not Arduino)
    if template == "arduino" {
        // Arduino uses .ino files in project root
        fs::write(format!("{}.ino", name), main_code)?;
    } else if !Path::new("src").exists() {
        fs::create_dir("src")?;
        let ext = if lang == "c" { "c" } else { "cpp" };
        fs::write(Path::new("src").join(format!("main.{}", ext)), main_code)?;
    } else {
        println!(
            "{} 'src' directory exists, skipping main file creation.",
            "!".yellow()
        );
    }

    // Write .gitignore if not exists
    if !Path::new(".gitignore").exists() {
        fs::write(".gitignore", ".cx/\nvendor/\n")?;
    }

    println!(
        "{} Initialized caxe project in current directory.",
        "✓".green()
    );
    Ok(())
}

fn create_project(name_opt: &Option<String>, lang_cli: &str, templ_cli: &str) -> Result<()> {
    // 1. Interactive Inputs
    let name = match name_opt {
        Some(n) => n.clone(),
        None => Text::new("What is your project name?")
            .with_default("my-app")
            .prompt()?,
    };

    let template = if name_opt.is_none() {
        let options = vec![
            "console", "arduino", "web", "raylib", "sdl2", "sdl3", "opengl",
        ];
        Select::new("Select a template:", options).prompt()?
    } else {
        templ_cli
    };

    let lang = if name_opt.is_none() {
        let options = vec!["cpp", "c"];
        Select::new("Select language:", options).prompt()?
    } else {
        lang_cli
    };

    // 2. Setup Directory
    let path = Path::new(&name);
    if path.exists() {
        println!("{} Error: Directory '{}' already exists", "x".red(), name);
        return Ok(());
    }

    fs::create_dir_all(path.join("src")).context("Failed to create src")?;

    // 3. Get Template Content (Refactored)
    // Use only the final directory name as project name (not the full path)
    let project_name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let (toml_content, main_code) = templates::get_template(&project_name, lang, template);

    // 4. Write Files
    fs::write(path.join("cx.toml"), toml_content)?;
    fs::write(path.join(".gitignore"), ".cx/\nvendor/\n")?;

    // Arduino uses .ino files in project root, other templates use src/main.cpp|c
    if template == "arduino" {
        fs::write(path.join(format!("{}.ino", project_name)), main_code)?;
    } else {
        let ext = if lang == "c" { "c" } else { "cpp" };
        fs::write(path.join("src").join(format!("main.{}", ext)), main_code)?;
    }

    // 5. VS Code Intellisense Support
    let vscode_dir = path.join(".vscode");
    fs::create_dir_all(&vscode_dir).context("Failed to create .vscode dir")?;

    let vscode_json = r#"{
    "configurations": [
        {
            "name": "cx-config",
            "includePath": ["${workspaceFolder}/**"],
            "compileCommands": "${workspaceFolder}/.cx/build/compile_commands.json",
            "cStandard": "c17",
            "cppStandard": "c++17"
        }
    ],
    "version": 4
}"#;
    fs::write(vscode_dir.join("c_cpp_properties.json"), vscode_json)?;

    // 6. Success Message
    println!(
        "{} Created new project: {} (template: {})",
        "✓".green(),
        name.bold(),
        template.cyan()
    );
    println!("  cd {}\n  cx run", name);
    Ok(())
}

fn print_info() -> Result<()> {
    println!("{} v{}", "caxe".bold().cyan(), env!("CARGO_PKG_VERSION"));
    println!("The Modern C/C++ Project Manager 🪓");
    println!("------------------------------------");

    // System Info
    println!(
        "{}: {} {}",
        "System".bold(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    // Cache Info
    let home = dirs::home_dir().unwrap_or_default();
    println!(
        "{}: {}",
        "Cache".bold(),
        home.join(".cx").join("cache").display()
    );

    println!("\n{}", "Available Toolchains:".bold());

    #[cfg(windows)]
    {
        use toolchain::CompilerType;
        use toolchain::windows::discover_all_toolchains;

        let toolchains = discover_all_toolchains();
        if toolchains.is_empty() {
            println!("  {} No toolchains found!", "x".red());
            println!("  Install Visual Studio Build Tools or LLVM to get started.");
        } else {
            // Check project's cx.toml for compiler preference
            let project_compiler = if Path::new("cx.toml").exists() {
                if let Ok(config) = build::load_config() {
                    config.build.and_then(|b| b.compiler)
                } else {
                    None
                }
            } else {
                None
            };

            // Also check cached toolchain selection from 'cx toolchain'
            let cached_selection = {
                let cache_path = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".cx")
                    .join("toolchain-selection.toml");

                if cache_path.exists() {
                    std::fs::read_to_string(&cache_path)
                        .ok()
                        .and_then(|content| {
                            // Parse compiler_type from the cached file
                            for line in content.lines() {
                                if line.starts_with("compiler_type") {
                                    if line.contains("MSVC") {
                                        return Some(CompilerType::MSVC);
                                    }
                                    if line.contains("ClangCL") {
                                        return Some(CompilerType::ClangCL);
                                    }
                                    if line.contains("Clang") {
                                        return Some(CompilerType::Clang);
                                    }
                                    if line.contains("GCC") {
                                        return Some(CompilerType::GCC);
                                    }
                                }
                            }
                            None
                        })
                } else {
                    None
                }
            };

            // Determine which compiler type is configured (cx.toml takes priority over cached selection)
            let configured_type = match project_compiler.as_deref() {
                Some("msvc") | Some("cl") | Some("cl.exe") => Some(CompilerType::MSVC),
                Some("clang-cl") | Some("clangcl") => Some(CompilerType::ClangCL),
                Some("clang") | Some("clang++") => Some(CompilerType::Clang),
                Some("gcc") | Some("g++") => Some(CompilerType::GCC),
                _ => cached_selection, // Fall back to cached selection
            };

            // Find which one is in use
            let in_use_idx = match &configured_type {
                Some(ct) => toolchains.iter().position(|tc| tc.compiler_type == *ct),
                None => Some(0), // Default is first
            };

            let mut table = ui::Table::new(&["Id", "Name", "Version", "Source"]);
            for (i, tc) in toolchains.iter().enumerate() {
                let is_in_use = in_use_idx == Some(i);

                let short_ver = if tc.version.len() > 30 {
                    format!("{}...", &tc.version[..30])
                } else {
                    tc.version.clone()
                };

                let mut row = vec![
                    format!("{}", i + 1),
                    tc.display_name.clone(),
                    short_ver,
                    tc.source.clone(),
                ];

                if is_in_use {
                    row = row
                        .into_iter()
                        .map(|s| s.green().bold().to_string())
                        .collect();
                } else {
                    row[0] = row[0].dimmed().to_string();
                    row[1] = row[1].cyan().to_string();
                    row[2] = row[2].dimmed().to_string();
                    row[3] = row[3].yellow().to_string();
                }

                table.add_row(row);
            }
            table.print();

            // Show current ABI and config source
            println!();
            println!("{}", "Current Configuration:".bold());

            let active_tc = in_use_idx.and_then(|i| toolchains.get(i));
            if let Some(tc) = active_tc {
                println!(
                    "  {}: {} ({})",
                    "Compiler".bold(),
                    tc.display_name.cyan(),
                    tc.source
                );
                let abi = if tc.path.to_string_lossy().contains("x64")
                    || tc.path.to_string_lossy().contains("Hostx64")
                {
                    "x86_64 (64-bit)"
                } else if tc.path.to_string_lossy().contains("x86")
                    || tc.path.to_string_lossy().contains("Hostx86")
                {
                    "x86 (32-bit)"
                } else {
                    "x86_64 (64-bit)"
                };
                println!("  {}: {}", "Target ABI".bold(), abi.cyan());
            }
            println!(
                "  {}: Set {} in cx.toml to override",
                "Override".bold(),
                "compiler = \"...\"".yellow()
            );
        }

        // Build tools check (cmake, make, etc.)
        println!("\n{}", "Build Tools:".bold());
        let mut table = ui::Table::new(&["Status", "Tool", "Version"]);
        let tools = vec![("cmake", "CMake"), ("make", "Make"), ("ninja", "Ninja")];
        for (bin, name) in tools {
            let output = std::process::Command::new(bin).arg("--version").output();
            let (status, version) = match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let first_line = stdout.lines().next().unwrap_or("Detected").trim();
                    let short = if first_line.len() > 40 {
                        &first_line[..40]
                    } else {
                        first_line
                    };
                    ("✓".green().to_string(), short.to_string())
                }
                _ => ("x".red().to_string(), "Not Found".dimmed().to_string()),
            };
            table.add_row(vec![status, name.to_string(), version]);
        }
        table.print();
    }

    #[cfg(not(windows))]
    {
        // Unix fallback - check PATH
        let compilers = vec![("clang++", "LLVM C++"), ("g++", "GNU C++")];

        let mut table = ui::Table::new(&["Status", "Binary", "Name", "Version"]);
        for (bin, name) in compilers {
            let output = std::process::Command::new(bin).arg("--version").output();
            let (status, version) = match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let first_line = stdout.lines().next().unwrap_or("Detected").trim();
                    ("✓".green().to_string(), first_line.to_string())
                }
                _ => ("x".red().to_string(), "Not Found".dimmed().to_string()),
            };
            table.add_row(vec![status, bin.to_string(), name.to_string(), version]);
        }
        table.print();
    }

    Ok(())
}
