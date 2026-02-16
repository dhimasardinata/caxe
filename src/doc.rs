use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;
use std::process::Command;

fn doxygen_exists() -> bool {
    Command::new("doxygen").arg("--version").output().is_ok()
}

fn ensure_doxyfile() -> Result<()> {
    if Path::new("Doxyfile").exists() {
        return Ok(());
    }

    println!("   Creating default Doxyfile...");
    let project_name = std::env::current_dir()?
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let doxy_content = format!(
        r#"PROJECT_NAME           = "{}"
OUTPUT_DIRECTORY       = docs
INPUT                  = src
RECURSIVE              = YES
GENERATE_HTML          = YES
GENERATE_LATEX         = NO
OPTIMIZE_OUTPUT_FOR_C  = YES
EXTRACT_ALL            = YES
"#,
        project_name
    );
    fs::write("Doxyfile", doxy_content)?;
    Ok(())
}

fn run_doxygen() -> Result<std::process::Output> {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.magenta} {msg}")
            .unwrap_or_else(|_| indicatif::ProgressStyle::default_spinner())
            .tick_chars("◜◠◝◞◡◟"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb.set_message("Running Doxygen...");

    let output = Command::new("doxygen").output()?;
    pb.finish_and_clear();
    Ok(output)
}

pub fn generate_docs() -> Result<()> {
    println!("{} Generating documentation...", "📚".magenta());

    if !doxygen_exists() {
        println!("{} Doxygen not found. Please install it first.", "x".red());
        return Ok(());
    }

    ensure_doxyfile()?;
    let output = run_doxygen()?;

    if output.status.success() {
        println!(
            "{} Documentation generated in docs/html/index.html",
            "✓".green()
        );
    } else {
        println!("{} Doxygen failed:", "x".red());
        println!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}
