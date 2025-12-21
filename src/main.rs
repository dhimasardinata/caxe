use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use colored::*;
use inquire::{Select, Text};
use std::fs;
use std::path::{Path, PathBuf};

mod build;
mod cache;
mod checker;
mod ci;
mod config;
mod deps;
mod doc;
mod docker;
mod ide;
mod import;
mod lock;
mod registry;
mod stats;
mod templates;
mod toolchain;
mod tree;
mod ui;
mod upgrade;

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
        /// Generate build profile (Chrome Tracing)
        #[arg(long)]
        profile: bool,
        /// Compile to WebAssembly (requires Emscripten)
        #[arg(long)]
        wasm: bool,
        /// Enable Link Time Optimization
        #[arg(long)]
        lto: bool,
        /// Enable Sanitizers (address, thread, undefined, leak)
        #[arg(long)]
        sanitize: Option<String>,
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
    Fmt,
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

        Some(Commands::Build {
            release,
            verbose,
            dry_run,
            profile,
            wasm,
            lto,
            sanitize,
        }) => {
            let config = build::load_config()?;
            let options = build::BuildOptions {
                release: *release,
                verbose: *verbose,
                dry_run: *dry_run,
                enable_profile: *profile,
                wasm: *wasm,
                lto: *lto,
                sanitize: sanitize.clone(),
            };
            build::build_project(&config, &options).map(|_| ())
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
        Some(Commands::Fmt) => checker::format_code(),
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
        vec!["console", "web", "raylib", "sdl2", "opengl"],
    )
    .prompt()?;

    let (toml_content, main_code) = templates::get_template(&name, lang, template);

    fs::write("cx.toml", toml_content)?;

    // Create src if generic template
    if !Path::new("src").exists() {
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
        fs::write(".gitignore", "/build\n/compile_commands.json\n")?;
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
        let options = vec!["console", "web", "raylib", "sdl2", "opengl"];
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
    let ext = if lang == "c" { "c" } else { "cpp" };
    fs::write(path.join("cx.toml"), toml_content)?;
    fs::write(path.join("src").join(format!("main.{}", ext)), main_code)?;
    fs::write(path.join(".gitignore"), "/build\n/compile_commands.json\n")?;

    // 5. VS Code Intellisense Support
    let vscode_dir = path.join(".vscode");
    fs::create_dir_all(&vscode_dir).context("Failed to create .vscode dir")?;

    let vscode_json = r#"{
    "configurations": [
        {
            "name": "cx-config",
            "includePath": ["${workspaceFolder}/**"],
            "compileCommands": "${workspaceFolder}/compile_commands.json",
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
        }
    }

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
