use crate::build::load_config;
use crate::deps;
use anyhow::Result;
use colored::*;
use std::process::Command;
use walkdir::WalkDir;

pub fn format_code() -> Result<()> {
    if Command::new("clang-format")
        .arg("--version")
        .output()
        .is_err()
    {
        println!(
            "{} clang-format not found. Please install it first.",
            "x".red()
        );
        return Ok(());
    }

    println!("{} Formatting source code...", "🎨".magenta());

    let mut count = 0;
    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if ["cpp", "hpp", "c", "h", "cc", "cxx"].contains(&s.as_ref()) {
                let status = Command::new("clang-format")
                    .arg("-i")
                    .arg("-style=file")
                    .arg(path)
                    .status()?;

                if status.success() {
                    count += 1;
                }
            }
        }
    }

    println!("{} Formatted {} files.", "✓".green(), count);
    Ok(())
}

pub fn check_code() -> Result<()> {
    if Command::new("clang-tidy")
        .arg("--version")
        .output()
        .is_err()
    {
        println!(
            "{} clang-tidy not found. Please install it first.",
            "x".red()
        );
        return Ok(());
    }

    println!("{} Checking code with clang-tidy...", "🔍".magenta());

    // Need config to get include paths and flags
    // Note: load_config is currently in builder, we might need to expose it or move it to generic config
    let config = load_config()?;

    // Fetch dependencies for include paths
    let mut include_flags = Vec::new();
    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            if let Ok((incs, _)) = deps::fetch_dependencies(deps) {
                include_flags = incs;
            }
        }
    }

    let mut count = 0;
    let mut warnings = 0;

    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if ["cpp", "hpp", "c", "h", "cc", "cxx"].contains(&s.as_ref()) {
                print!(
                    "   Checking {} ... ",
                    path.file_name().unwrap().to_string_lossy()
                );

                let mut cmd = Command::new("clang-tidy");
                cmd.arg(path);
                cmd.arg("--"); // Separator for compiler flags
                cmd.arg(format!("-std={}", config.package.edition));

                if let Some(build_cfg) = &config.build {
                    if let Some(flags) = &build_cfg.cflags {
                        cmd.args(flags);
                    }
                }
                cmd.args(&include_flags);

                let output = cmd.output()?;
                if output.status.success() {
                    // Check if output contains "warning:" or "error:"
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("warning:") || stdout.contains("error:") {
                        println!("{}", "!".yellow());
                        println!("{}", stdout);
                        warnings += 1;
                    } else {
                        println!("{}", "✓".green());
                    }
                } else {
                    println!("{}", "x".red());
                    println!("{}", String::from_utf8_lossy(&output.stderr));
                    warnings += 1;
                }
                count += 1;
            }
        }
    }

    if warnings == 0 {
        println!("{} Checked {} files. No issues found.", "✓".green(), count);
    } else {
        println!(
            "{} Checked {} files. Found issues in {} files.",
            "!".yellow(),
            count,
            warnings
        );
    }

    Ok(())
}
