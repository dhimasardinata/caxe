use crate::build::load_config;
use crate::deps;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::process::Command;
use walkdir::WalkDir;

pub fn format_code(check_only: bool) -> Result<()> {
    use std::fs;
    use std::path::Path;

    if Command::new("clang-format")
        .arg("--version")
        .output()
        .is_err()
    {
        println!("{} clang-format not found.", "x".red());
        println!(
            "   {} Run {} to install it.",
            "💡".yellow(),
            "cx toolchain install".cyan()
        );
        return Ok(());
    }

    // Auto-create .clang-format if missing (zero-config philosophy)
    let clang_format_path = Path::new(".clang-format");
    if !clang_format_path.exists() {
        let default_style = r#"---
BasedOnStyle: Google
IndentWidth: 4
ColumnLimit: 100
AllowShortFunctionsOnASingleLine: Empty
AllowShortIfStatementsOnASingleLine: false
AllowShortLoopsOnASingleLine: false
BreakBeforeBraces: Attach
IndentCaseLabels: true
PointerAlignment: Left
SpaceAfterCStyleCast: false
SpacesBeforeTrailingComments: 2
"#;
        fs::write(clang_format_path, default_style)?;
        println!(
            "{} Created {} with sensible defaults",
            "✓".green(),
            ".clang-format".cyan()
        );
    }

    let mode_msg = if check_only {
        "Checking formatting..."
    } else {
        "Formatting source code..."
    };
    println!("{} {}", "🎨".magenta(), mode_msg);

    let mut files = Vec::new();
    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if ["cpp", "hpp", "c", "h", "cc", "cxx"].contains(&s.as_ref()) {
                files.push(path);
            }
        }
    }

    // Also check include/ directory if it exists
    if Path::new("include").exists() {
        for entry in WalkDir::new("include").into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();
            if let Some(ext) = path.extension() {
                let s = ext.to_string_lossy();
                if ["cpp", "hpp", "c", "h", "cc", "cxx"].contains(&s.as_ref()) {
                    files.push(path);
                }
            }
        }
    }

    if files.is_empty() {
        println!("{} No source files found to format.", "!".yellow());
        return Ok(());
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut formatted_count = 0;
    let mut unformatted_files = Vec::new();

    for path in &files {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        pb.set_message(format!("Checking {}", name));

        if check_only {
            // Check mode: use --dry-run and --Werror to detect unformatted files
            let output = Command::new("clang-format")
                .arg("--dry-run")
                .arg("--Werror")
                .arg("-style=file")
                .arg(path)
                .output();

            if let Ok(out) = output
                && (!out.status.success() || !out.stderr.is_empty()) {
                    unformatted_files.push(path.display().to_string());
                }
        } else {
            // Format mode: apply formatting in-place
            let status = Command::new("clang-format")
                .arg("-i")
                .arg("-style=file")
                .arg(path)
                .status();

            if let Ok(s) = status
                && s.success()
            {
                formatted_count += 1;
            }
        }
        pb.inc(1);
    }

    pb.finish_and_clear();

    if check_only {
        if unformatted_files.is_empty() {
            println!(
                "{} All {} files are properly formatted.",
                "✓".green(),
                files.len()
            );
            Ok(())
        } else {
            println!(
                "{} {} file(s) need formatting:",
                "x".red(),
                unformatted_files.len()
            );
            for file in &unformatted_files {
                println!("   {}", file.yellow());
            }
            println!("\n   Run {} to fix formatting.", "cx fmt".cyan().bold());
            std::process::exit(1);
        }
    } else {
        println!("{} Formatted {} files.", "✓".green(), formatted_count);
        Ok(())
    }
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

    let config = load_config()?;

    // Fetch dependencies for include paths
    let mut include_flags = Vec::new();
    if let Some(deps) = &config.dependencies
        && !deps.is_empty()
        && let Ok((paths, cflags, _)) = deps::fetch_dependencies(deps)
    {
        for p in paths {
            include_flags.push(format!("-I{}", p.display()));
        }
        include_flags.extend(cflags);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if ["cpp", "hpp", "c", "h", "cc", "cxx"].contains(&s.as_ref()) {
                files.push(path);
            }
        }
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let warnings: usize = files
        .par_iter()
        .map(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            pb.set_message(format!("Checking {}", name));

            let mut cmd = Command::new("clang-tidy");
            cmd.arg(path);
            cmd.arg("--");
            cmd.arg(format!("-std={}", config.package.edition));

            if let Some(build_cfg) = &config.build
                && let Some(flags) = &build_cfg.cflags
            {
                cmd.args(flags);
            }
            cmd.args(&include_flags);

            // Execute clang-tidy
            let output = cmd.output().ok(); // Handle potential execution failure gracefully

            if let Some(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let has_issues = stdout.contains("warning:")
                    || stdout.contains("error:")
                    || !out.status.success();

                if has_issues {
                    pb.suspend(|| {
                        println!("{} Issues in {}", "!".yellow(), name);
                        if !stdout.is_empty() {
                            println!("{}", stdout.trim());
                        }
                        if !stderr.is_empty() {
                            println!("{}", stderr.trim());
                        }
                        println!("{}", "-".repeat(40).dimmed());
                    });
                    pb.inc(1);
                    return 1;
                }
            }

            pb.inc(1);
            0
        })
        .sum();

    pb.finish_and_clear();

    if warnings == 0 {
        println!(
            "{} Checked {} files. No issues found.",
            "✓".green(),
            files.len()
        );
    } else {
        println!(
            "{} Checked {} files. Found issues in {} files.",
            "!".yellow(),
            files.len(),
            warnings
        );
    }

    Ok(())
}
