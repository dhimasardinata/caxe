use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use inquire::{Select, Text};
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
        name: Option<String>,
        #[arg(long, default_value = "cpp")]
        lang: String,
        #[arg(long, default_value = "console")]
        template: String,
    },
    Build {
        #[arg(long)]
        release: bool,
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
    Remove {
        lib: String,
    },
    Watch,
    Clean,
    Test,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::New {
            name,
            lang,
            template,
        } => create_project(name, lang, template),
        Commands::Build { release } => builder::build_project(*release).map(|_| ()),
        Commands::Run { release, args } => builder::build_and_run(*release, args),
        Commands::Watch => builder::watch(),
        Commands::Clean => builder::clean(),
        Commands::Test => builder::run_tests(),
        Commands::Add { lib } => deps::add_dependency(lib),
        Commands::Remove { lib } => deps::remove_dependency(lib),
    }
}

// src/main.rs (Ganti fungsi create_project yang lama dengan ini)

fn create_project(name_opt: &Option<String>, lang_cli: &str, templ_cli: &str) -> Result<()> {
    let name = match name_opt {
        Some(n) => n.clone(),
        None => Text::new("What is your project name?")
            .with_default("my-app")
            .prompt()?,
    };
    let template = if name_opt.is_none() {
        let options = vec!["console", "web", "raylib"];
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

    let path = Path::new(&name);
    if path.exists() {
        println!("{} Error: Directory '{}' already exists", "x".red(), name);
        return Ok(());
    }

    fs::create_dir_all(path.join("src")).context("Failed to create src")?;

    let (toml_content, main_code) = match template {
        "raylib" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
libs = ["raylib", "gdi32", "user32", "shell32", "winmm", "opengl32"]

[dependencies]
raylib = "https://github.com/raysan5/raylib.git"
"#,
                name
            ),
            r#"#include "raylib.h"
int main() {
    InitWindow(800, 600, "cx + raylib");
    SetTargetFPS(60);
    while (!WindowShouldClose()) {
        BeginDrawing();
        ClearBackground(RAYWHITE);
        DrawText("Hello Raylib!", 190, 200, 20, LIGHTGRAY);
        EndDrawing();
    }
    CloseWindow();
    return 0;
}
"#,
        ),
        "web" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
cflags = ["-D_WIN32_WINNT=0x0A00"]
libs = ["ws2_32"]

[dependencies]
httplib = "https://github.com/yhirose/cpp-httplib.git"
"#,
                name
            ),
            r#"#include <iostream>
#include "httplib.h"
int main() {
    httplib::Server svr;
    svr.Get("/", [](const httplib::Request&, httplib::Response& res) {
        res.set_content("<h1>Hello from cx template!</h1>", "text/html");
    });
    std::cout << "Server at http://localhost:8080" << std::endl;
    svr.listen("0.0.0.0", 8080);
    return 0;
}
"#,
        ),
        _ => {
            let dep = if lang == "cpp" {
                "\n[dependencies]\n# json = \"...\""
            } else {
                ""
            };
            let cfg = format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "{}"
{}
"#,
                name,
                if lang == "c" { "c17" } else { "c++20" },
                dep
            );

            let code = if lang == "c" {
                "#include <stdio.h>\nint main() { printf(\"Hello cx!\\n\"); return 0; }"
            } else {
                "#include <iostream>\nint main() { std::cout << \"Hello cx!\" << std::endl; return 0; }"
            };
            (cfg, code)
        }
    };

    fs::write(path.join("cx.toml"), toml_content)?;
    let ext = if lang == "c" { "c" } else { "cpp" };
    fs::write(path.join("src").join(format!("main.{}", ext)), main_code)?;
    fs::write(path.join(".gitignore"), "/build\n")?;

    println!(
        "{} Created new project: {} (template: {})",
        "✓".green(),
        name.bold(),
        template.cyan()
    );
    println!("  cd {}\n  cx run", name);
    Ok(())
}
