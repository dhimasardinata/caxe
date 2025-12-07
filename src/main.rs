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
#[command(about = "The modern C/C++ project manager", version = env!("CARGO_PKG_VERSION"))]
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
    Info,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::New {
            name,
            lang,
            template,
        } => create_project(name, lang, template),

        Commands::Build { release } => {
            let config = builder::load_config()?;
            builder::build_project(&config, *release).map(|_| ())
        }

        Commands::Run { release, args } => builder::build_and_run(*release, args),

        Commands::Watch => builder::watch(),
        Commands::Clean => builder::clean(),
        Commands::Test => builder::run_tests(),
        Commands::Add { lib } => deps::add_dependency(lib),
        Commands::Remove { lib } => deps::remove_dependency(lib),
        Commands::Info => print_info(),
    }
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

    // 2. Setup Directory
    let path = Path::new(&name);
    if path.exists() {
        println!("{} Error: Directory '{}' already exists", "x".red(), name);
        return Ok(());
    }

    fs::create_dir_all(path.join("src")).context("Failed to create src")?;

    // 3. Get Template Content (Refactored)
    let (toml_content, main_code) = get_template(&name, lang, template);

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

    println!("\n{}", "Toolchain Check:".bold());
    let compilers = vec![
        ("clang++", "LLVM C++"),
        ("g++", "GNU C++"),
        ("gcc", "GNU C"),
        ("cl", "MSVC"),
        ("cmake", "CMake"),
        ("make", "Make"),
    ];

    for (bin, name) in compilers {
        let output = std::process::Command::new(bin).arg("--version").output();
        let (status, version) = match output {
            Ok(out) => {
                let v_str = String::from_utf8_lossy(&out.stdout);
                let first_line = v_str.lines().next().unwrap_or("Detected").trim();
                let short_ver = if first_line.len() > 40 {
                    &first_line[..40]
                } else {
                    first_line
                };
                ("✓".green(), short_ver.to_string())
            }
            Err(_) => ("x".red(), "Not Found".dimmed().to_string()),
        };
        println!("  [{}] {:<10} : ({}) {}", status, bin, name, version);
    }

    Ok(())
}

// --- Template Helper ---
fn get_template(name: &str, lang: &str, template: &str) -> (String, String) {
    match template {
        "raylib" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
libs = ["gdi32", "user32", "shell32", "winmm", "opengl32"]

[dependencies]
raylib = {{ git = "https://github.com/raysan5/raylib.git", build = "mingw32-make -C src PLATFORM=PLATFORM_DESKTOP", output = "src/libraylib.a" }}
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
"#
            .to_string(),
        ),
        "web" => {
            if lang == "c" {
                (
                    format!(
                        r#"[package]
name = "{}"
version = "0.1.0"
edition = "c17"

[build]
libs = ["ws2_32"]

[dependencies]
mongoose = {{ git = "https://github.com/cesanta/mongoose.git", build = "clang -c mongoose.c -o libmongoose.a", output = "libmongoose.a" }}
"#,
                        name
                    ),
                    r#"#include "mongoose.h"

static void fn(struct mg_connection* c, int ev, void* ev_data) {
  if (ev == MG_EV_HTTP_MSG) {
    mg_http_reply(c, 200, "", "<h1>Hello from C (Mongoose)!</h1>\n");
  }
}

int main() {
  struct mg_mgr mgr;
  mg_mgr_init(&mgr);
  printf("Server running at http://localhost:8000\n");
  mg_http_listen(&mgr, "http://0.0.0.0:8000", fn, NULL);
  for (;;) mg_mgr_poll(&mgr, 1000);
  mg_mgr_free(&mgr);
  return 0;
}
"#
                    .to_string(),
                )
            } else {
                (
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
        res.set_content("<h1>Hello from cx (C++)!</h1>", "text/html");
    });
    std::cout << "Server at http://localhost:8080" << std::endl;
    svr.listen("0.0.0.0", 8080);
    return 0;
}
"#
                    .to_string(),
                )
            }
        }
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
            (cfg, code.to_string())
        }
    }
}
