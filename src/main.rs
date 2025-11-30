use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::fs;
use std::path::Path;

mod builder;
mod config;
mod deps;

#[derive(Parser)]
#[command(name = "cx")]
#[command(about = "The modern C/C++ project manager", version = "0.2.0")]
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
    Add {
        lib: String,
    },
    Watch,
    Clean,
    Test,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::New { name, lang } => create_project(name, lang),
        Commands::Run { release, args } => builder::build_and_run(*release, args),
        Commands::Watch => builder::watch(),
        Commands::Clean => builder::clean(),
        Commands::Test => builder::run_tests(),
        Commands::Add { lib } => deps::add_dependency(lib),
    }
}

fn create_project(name: &str, lang: &str) -> Result<()> {
    let path = Path::new(name);
    if path.exists() {
        println!("{} Error: Directory '{}' already exists", "x".red(), name);
        return Ok(());
    }

    fs::create_dir_all(path.join("src")).context("Failed to create src")?;

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
