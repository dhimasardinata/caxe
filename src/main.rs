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
use caxe::deps;
use caxe::doc;
use caxe::docker;
use caxe::ide;
use caxe::import;
use caxe::lock;
use caxe::package;
use caxe::registry;
use caxe::stats;
use caxe::templates;
use caxe::toolchain;
use caxe::tree;
use caxe::ui;
use caxe::upgrade;

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
    /// Manage cross-compilation targets
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
    /// Add a target to the project
    Add {
        /// Target name (windows-x64, linux-x64, macos-x64, wasm32, esp32)
        name: String,
    },
    /// Remove a target from the project
    Remove {
        /// Target name
        name: String,
    },
    /// Set the default target
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

fn main() -> Result<()> {
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
            handle_lock(*update, *check);
            Ok(())
        }

        Some(Commands::Sync) => {
            handle_sync();
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
                build::build_project(&config, &options).map(|_| ())
            }
        }

        Some(Commands::Run {
            release,
            verbose,
            dry_run,
            args,
        }) => build::build_and_run(*release, *verbose, *dry_run, args.clone(), None),

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
        Some(Commands::Toolchain { op }) => handle_toolchain_command(op),
        Some(Commands::Doctor) => run_doctor(),
        Some(Commands::Vendor) => deps::vendor_dependencies(),
        Some(Commands::CI) => ci::generate_ci_config(),
        Some(Commands::Docker) => docker::generate_docker_config(),
        Some(Commands::SetupIde) => ide::generate_ide_config(),
        Some(Commands::Tree) => tree::print_tree(),
        Some(Commands::Stats) => stats::print_stats(),
        Some(Commands::Target { op }) => handle_target_command(op),
        Some(Commands::Generate { format }) => handle_generate_command(format),
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
        let options = vec!["console", "arduino", "web", "raylib", "sdl2", "opengl"];
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

fn handle_toolchain_command(_op: &Option<ToolchainOp>) -> Result<()> {
    #[cfg(windows)]
    {
        use toolchain::windows::discover_all_toolchains;

        match _op {
            Some(ToolchainOp::List) => {
                let toolchains = discover_all_toolchains();
                if toolchains.is_empty() {
                    println!("{} No toolchains found.", "x".red());
                } else {
                    // Try to detect active toolchain to highlight it
                    let config = crate::build::load_config().ok();
                    let preferred_type = config
                        .as_ref()
                        .and_then(|c| c.build.as_ref())
                        .and_then(|b| b.compiler.as_ref())
                        .map(|s| match s.as_str() {
                            "clang-cl" => toolchain::CompilerType::ClangCL,
                            "clang" => toolchain::CompilerType::Clang,
                            "g++" | "gcc" => toolchain::CompilerType::GCC,
                            _ => toolchain::CompilerType::MSVC,
                        });

                    let active = toolchain::get_or_detect_toolchain(preferred_type, false).ok();

                    println!("{} Available Toolchains:", "Available Toolchains:".bold());
                    let mut table = crate::ui::Table::new(&["Id", "Name", "Version", "Source"]);

                    for (i, tc) in toolchains.iter().enumerate() {
                        let is_in_use = if let Some(a) = &active {
                            tc.path == a.cc_path || tc.path == a.cxx_path
                        } else {
                            false
                        };

                        let short_ver = if tc.version.len() > 40 {
                            format!("{}...", &tc.version[..40])
                        } else {
                            tc.version.clone()
                        };

                        let mut row = vec![
                            format!("{}", i + 1),
                            tc.display_name.clone(),
                            short_ver,
                            tc.source.to_string(),
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
                }
            }

            None | Some(ToolchainOp::Select) => {
                // Interactive selection (default behavior)
                let toolchains = discover_all_toolchains();
                if toolchains.is_empty() {
                    println!("{} No toolchains found!", "x".red());
                    println!("  Install Visual Studio Build Tools or LLVM to get started.");
                    return Ok(());
                }

                // Format options for display
                let options: Vec<String> = toolchains.iter().map(|tc| tc.to_string()).collect();

                let selection = Select::new("Select a toolchain:", options).prompt()?;

                // Find the selected toolchain
                let selected = toolchains.iter().find(|tc| tc.to_string() == selection);

                if let Some(tc) = selected {
                    // Cache the selection
                    let cache_path = dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".cx")
                        .join("toolchain-selection.toml");

                    let content = format!(
                        "# User-selected toolchain\ncompiler_type = {:?}\npath = {:?}\nversion = {:?}\nsource = {:?}\n",
                        format!("{:?}", tc.compiler_type),
                        tc.path.display(),
                        tc.version,
                        tc.source
                    );

                    if let Err(e) = std::fs::create_dir_all(cache_path.parent().unwrap()) {
                        println!("{} Failed to create cache dir: {}", "x".red(), e);
                    } else if let Err(e) = std::fs::write(&cache_path, content) {
                        println!("{} Failed to save selection: {}", "x".red(), e);
                    } else {
                        println!();
                        println!(
                            "{} Selected: {} ({})",
                            "✓".green(),
                            tc.display_name.cyan(),
                            tc.source.yellow()
                        );
                        println!("  Saved to: {}", cache_path.display().to_string().dimmed());
                    }

                    // Also update cx.toml if we're in a project
                    if Path::new("cx.toml").exists() {
                        let compiler_str = match tc.compiler_type {
                            toolchain::CompilerType::MSVC => "msvc",
                            toolchain::CompilerType::ClangCL => "clang-cl",
                            toolchain::CompilerType::Clang => "clang",
                            toolchain::CompilerType::GCC => "g++",
                        };

                        // Read current cx.toml
                        if let Ok(toml_content) = std::fs::read_to_string("cx.toml") {
                            let new_content = if toml_content.contains("[build]") {
                                // Update existing [build] section
                                if toml_content.contains("compiler =") {
                                    // Replace existing compiler line
                                    let mut result = String::new();
                                    for line in toml_content.lines() {
                                        if line.trim().starts_with("compiler =") {
                                            result.push_str(&format!(
                                                "compiler = \"{}\"",
                                                compiler_str
                                            ));
                                        } else {
                                            result.push_str(line);
                                        }
                                        result.push('\n');
                                    }
                                    result
                                } else {
                                    // Add compiler to existing [build] section
                                    toml_content.replace(
                                        "[build]",
                                        &format!("[build]\ncompiler = \"{}\"", compiler_str),
                                    )
                                }
                            } else {
                                // Add new [build] section
                                format!(
                                    "{}\n[build]\ncompiler = \"{}\"\n",
                                    toml_content.trim_end(),
                                    compiler_str
                                )
                            };

                            if let Err(e) = std::fs::write("cx.toml", new_content) {
                                println!("{} Failed to update cx.toml: {}", "x".red(), e);
                            } else {
                                println!(
                                    "  {} Updated cx.toml with compiler = \"{}\"",
                                    "✓".green(),
                                    compiler_str.cyan()
                                );
                            }
                        }
                    }
                }
            }

            Some(ToolchainOp::Clear) => {
                // Clear cached selection
                let cache_path = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".cx")
                    .join("toolchain-selection.toml");

                if cache_path.exists() {
                    if let Err(e) = std::fs::remove_file(&cache_path) {
                        println!("{} Failed to clear selection: {}", "x".red(), e);
                    } else {
                        println!("{} Cleared toolchain selection", "✓".green());
                    }
                } else {
                    println!("{} No selection cached.", "!".yellow());
                }
            }

            Some(ToolchainOp::Install { name }) => {
                toolchain::install::install_toolchain(name.clone())?;
            }

            Some(ToolchainOp::Update) => {
                toolchain::install::update_toolchains()?;
            }
        }
    }

    #[cfg(not(windows))]
    {
        println!(
            "{} Toolchain management is currently Windows-only.",
            "!".yellow()
        );
    }

    Ok(())
}

/// Handle the `cx target` command for cross-compilation targets
fn handle_target_command(op: &Option<TargetOp>) -> Result<()> {
    let config_path = Path::new("cx.toml");

    match op {
        None | Some(TargetOp::List) => {
            println!(
                "{} {}",
                "🎯".cyan(),
                "Available Cross-Compilation Targets".bold()
            );
            println!("{}", "─".repeat(50).dimmed());
            println!();
            println!(
                "   {} (MSVC) - Windows 64-bit",
                "windows-x64".green().bold()
            );
            println!(
                "   {} (MinGW) - Windows 64-bit GNU",
                "windows-x64-gnu".green()
            );
            println!("   {} (GCC/Clang) - Linux 64-bit", "linux-x64".blue());
            println!("   {} (Cross) - Linux ARM64", "linux-arm64".blue());
            println!("   {} (Clang) - macOS Intel", "macos-x64".magenta());
            println!(
                "   {} (Clang) - macOS Apple Silicon",
                "macos-arm64".magenta()
            );
            println!("   {} (Emscripten) - WebAssembly", "wasm32".yellow());
            println!("   {} (ESP-IDF) - ESP32 Microcontroller", "esp32".red());
            println!();

            // Show configured targets if in a project
            if config_path.exists()
                && let Ok(content) = std::fs::read_to_string(config_path)
            {
                if content.contains("[targets]") || content.contains("targets =") {
                    println!("{} Project targets configured", "✓".green());
                } else {
                    println!(
                        "{} No targets configured. Use {} to add one.",
                        "!".yellow(),
                        "cx target add <name>".cyan()
                    );
                }
            }
            println!();
            println!(
                "Usage: {} or {}",
                "cx target add <name>".cyan(),
                "cx build --target <name>".cyan()
            );
        }
        Some(TargetOp::Add { name }) => {
            if !config_path.exists() {
                println!(
                    "{} No cx.toml found. Run {} first.",
                    "x".red(),
                    "cx init".cyan()
                );
                return Ok(());
            }

            let valid_targets = [
                "windows-x64",
                "windows-x64-gnu",
                "linux-x64",
                "linux-arm64",
                "macos-x64",
                "macos-arm64",
                "wasm32",
                "esp32",
            ];

            if !valid_targets.contains(&name.as_str()) {
                println!(
                    "{} Unknown target '{}'. Run {} to see available targets.",
                    "x".red(),
                    name,
                    "cx target list".cyan()
                );
                return Ok(());
            }

            // Read and update config
            let mut content = std::fs::read_to_string(config_path)?;

            if content.contains(&format!("\"{}\"", name)) {
                println!("{} Target '{}' already configured.", "!".yellow(), name);
                return Ok(());
            }

            // Add targets section if not present
            if !content.contains("[targets]") {
                content.push_str(&format!("\n[targets]\nlist = [\"{}\"]\n", name));
            } else {
                // Append to existing targets list
                content = content.replace("list = [", &format!("list = [\"{}\", ", name));
            }

            std::fs::write(config_path, content)?;
            println!("{} Added target: {}", "✓".green(), name.cyan());
            println!(
                "   Build with: {}",
                format!("cx build --target {}", name).yellow()
            );
        }
        Some(TargetOp::Remove { name }) => {
            if !config_path.exists() {
                println!("{} No cx.toml found.", "x".red());
                return Ok(());
            }

            let content = std::fs::read_to_string(config_path)?;
            let new_content = content
                .replace(&format!("\"{}\", ", name), "")
                .replace(&format!(", \"{}\"", name), "")
                .replace(&format!("\"{}\"", name), "");

            std::fs::write(config_path, new_content)?;
            println!("{} Removed target: {}", "✓".green(), name);
        }
        Some(TargetOp::Default { name }) => {
            if !config_path.exists() {
                println!("{} No cx.toml found.", "x".red());
                return Ok(());
            }

            let mut content = std::fs::read_to_string(config_path)?;

            // Add or update default_target
            if content.contains("default_target") {
                // Replace existing
                let re = regex::Regex::new(r#"default_target\s*=\s*"[^"]*""#).unwrap();
                content = re
                    .replace(&content, &format!("default_target = \"{}\"", name))
                    .to_string();
            } else if content.contains("[targets]") {
                content = content.replace(
                    "[targets]",
                    &format!("[targets]\ndefault_target = \"{}\"", name),
                );
            } else {
                content.push_str(&format!("\n[targets]\ndefault_target = \"{}\"\n", name));
            }

            std::fs::write(config_path, content)?;
            println!("{} Set default target: {}", "✓".green(), name.cyan());
        }
    }
    Ok(())
}

/// Handle the `cx generate` command for build system file generation
fn handle_generate_command(format: &GenerateFormat) -> Result<()> {
    let config = build::load_config()?;

    match format {
        GenerateFormat::Cmake => {
            generate_cmake(&config)?;
        }
        GenerateFormat::Ninja => {
            generate_ninja(&config)?;
        }
        GenerateFormat::CompileCommands => {
            println!(
                "{} compile_commands.json is generated automatically when building.",
                "!".yellow()
            );
            println!("   Location: {}", ".cx/build/compile_commands.json".cyan());
            println!("   Run {} to generate it.", "cx build".cyan());
        }
    }
    Ok(())
}

fn generate_cmake(config: &caxe::config::CxConfig) -> Result<()> {
    println!("{} Generating CMakeLists.txt...", "📝".cyan());

    let name = &config.package.name;
    let edition = &config.package.edition;

    // Convert edition to CMake standard
    let cpp_standard = edition.replace("c++", "").replace("c", "");

    let mut cmake = format!(
        r#"cmake_minimum_required(VERSION 3.16)
project({name} LANGUAGES CXX)

set(CMAKE_CXX_STANDARD {cpp_standard})
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Source files
file(GLOB_RECURSE SOURCES "src/*.cpp" "src/*.c")

# Executable
add_executable(${{PROJECT_NAME}} ${{SOURCES}})

# Include directories
target_include_directories(${{PROJECT_NAME}} PRIVATE src)
"#
    );

    // Add dependencies if present
    if let Some(deps) = &config.dependencies {
        cmake.push_str("\n# Dependencies\n");
        for dep_name in deps.keys() {
            cmake.push_str(&format!("# find_package({} REQUIRED)\n", dep_name));
        }
    }

    // Add libs if present
    if let Some(build) = &config.build
        && let Some(libs) = &build.libs
    {
        cmake.push_str("\n# Libraries\ntarget_link_libraries(${PROJECT_NAME} PRIVATE");
        for lib in libs {
            cmake.push_str(&format!(" {}", lib));
        }
        cmake.push_str(")\n");
    }

    std::fs::write("CMakeLists.txt", cmake)?;
    println!("{} Created CMakeLists.txt", "✓".green());
    println!();
    println!("Usage:");
    println!(
        "   {} && {}",
        "cmake -B build -S .".yellow(),
        "cmake --build build".yellow()
    );

    Ok(())
}

fn generate_ninja(config: &caxe::config::CxConfig) -> Result<()> {
    println!("{} Generating build.ninja...", "📝".cyan());

    let name = &config.package.name;
    let edition = &config.package.edition;

    // Detect compiler
    let compiler = if cfg!(windows) { "cl" } else { "g++" };
    let is_msvc = compiler == "cl";

    let std_flag = if is_msvc {
        build::utils::get_std_flag_msvc(edition)
    } else {
        build::utils::get_std_flag_gcc(edition)
    };

    let mut ninja = String::from("# Auto-generated by caxe\n\n");

    if is_msvc {
        ninja.push_str(&format!(
            r#"
cxx = cl
cxxflags = /nologo /EHsc {std_flag} /c
linkflags = /nologo

rule compile
  command = $cxx $cxxflags $in /Fo$out
  description = Compiling $in

rule link
  command = $cxx $linkflags $in /Fe$out
  description = Linking $out

"#
        ));
    } else {
        ninja.push_str(&format!(
            r#"
cxx = g++
cxxflags = {std_flag} -c
linkflags = 

rule compile
  command = $cxx $cxxflags $in -o $out
  description = Compiling $in

rule link
  command = $cxx $linkflags $in -o $out
  description = Linking $out

"#
        ));
    }

    // Find source files
    let src_dir = Path::new("src");
    let mut obj_files = Vec::new();

    if src_dir.exists() {
        for entry in walkdir::WalkDir::new(src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| ["cpp", "cc", "cxx", "c"].contains(&e.to_str().unwrap()))
            {
                let obj_name = path.file_stem().unwrap().to_string_lossy();
                let obj_ext = if is_msvc { "obj" } else { "o" };
                let obj_path = format!("build/{}.{}", obj_name, obj_ext);

                ninja.push_str(&format!("build {}: compile {}\n", obj_path, path.display()));
                obj_files.push(obj_path);
            }
        }
    }

    // Link
    let exe_ext = if cfg!(windows) { ".exe" } else { "" };
    ninja.push_str(&format!(
        "\nbuild build/{}{}: link {}\n",
        name,
        exe_ext,
        obj_files.join(" ")
    ));
    ninja.push_str(&format!("\ndefault build/{}{}\n", name, exe_ext));

    std::fs::write("build.ninja", ninja)?;
    println!("{} Created build.ninja", "✓".green());
    println!();
    println!("Usage: {}", "ninja".yellow());

    Ok(())
}

fn run_doctor() -> Result<()> {
    println!("{} Running System Doctor...", "🚑".red());
    println!("-------------------------------");

    print!("Checking OS... ");
    println!(
        "{} ({})",
        std::env::consts::OS.green(),
        std::env::consts::ARCH.cyan()
    );

    #[cfg(windows)]
    {
        print!("Checking MSVC... ");
        let toolchains = toolchain::windows::discover_all_toolchains();
        if !toolchains.is_empty() {
            println!("{}", "Found".green());
            for tc in toolchains {
                println!("  - {} ({})", tc.display_name, tc.version);
            }
        } else {
            println!("{}", "Not Found (Install Visual Studio Build Tools)".red());
        }
    }

    print!("Checking Git... ");
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok()
    {
        println!("{}", "Found".green());
    } else {
        println!("{}", "Not Found (Install Git)".red());
    }

    // Check CMake
    print!("Checking CMake... ");
    if std::process::Command::new("cmake")
        .arg("--version")
        .output()
        .is_ok()
    {
        println!("{}", "Found".green());
    } else {
        println!("{}", "Not Found (Optional)".yellow());
    }

    Ok(())
}

fn handle_lock(update: bool, check: bool) {
    if check {
        println!("{} Verifying lockfile...", "🔒".blue());
        match lock::LockFile::load() {
            Ok(lockfile) => match build::load_config() {
                Ok(config) => {
                    let mut success = true;
                    if let Some(deps) = config.dependencies {
                        for (name, _) in deps {
                            if lockfile.get(&name).is_none() {
                                println!(
                                    "{} Dependency '{}' missing from cx.lock",
                                    "x".red(),
                                    name
                                );
                                success = false;
                            }
                        }
                    }
                    if success {
                        println!("{} Lockfile is in sync.", "✓".green());
                    } else {
                        println!(
                            "{} Lockfile out of sync. Run 'cx lock --update'.",
                            "x".red()
                        );
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error loading config: {}", e);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("Error loading lockfile: {}", e);
                std::process::exit(1);
            }
        }
    } else if update {
        println!("{} Updating lockfile...", "🔄".blue());
        if let Err(e) = deps::update_dependencies() {
            eprintln!("Error updating dependencies: {}", e);
            std::process::exit(1);
        }
    } else {
        println!("Use --check to verify or --update to update/regenerate.");
    }
}

fn handle_sync() {
    println!(
        "{} Synchronizing dependencies with lockfile...",
        "📦".blue()
    );
    // 1. Load Config to check if we even have deps
    match build::load_config() {
        Ok(config) => {
            if let Some(deps) = config.dependencies {
                // 2. Fetch/Sync
                // fetch_dependencies handles reading cx.lock and checking out specific revisions
                match deps::fetch_dependencies(&deps) {
                    Ok(_) => println!("{} Dependencies synchronized.", "✓".green()),
                    Err(e) => {
                        eprintln!("Error synchronizing: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("No dependencies found in cx.toml.");
            }
        }
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    }
}
