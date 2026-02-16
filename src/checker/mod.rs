//! Code quality tools: formatting and static analysis.
//!
//! This module provides the `cx fmt` and `cx check` commands for maintaining
//! code quality in C/C++ projects.
//!
//! ## Commands
//!
//! - `cx fmt` - Format code using clang-format
//! - `cx fmt --check` - Check formatting without modifying files
//! - `cx check` - Run static analysis using clang-tidy

use crate::build::load_config;
use crate::deps;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const SOURCE_EXTENSIONS: [&str; 6] = ["cpp", "hpp", "c", "h", "cc", "cxx"];

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            let ext_str = ext.to_string_lossy();
            SOURCE_EXTENSIONS.contains(&ext_str.as_ref())
        })
        .unwrap_or(false)
}

fn collect_source_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in ["src", "include"] {
        if root == "include" && !Path::new(root).exists() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();
            if is_source_file(&path) {
                files.push(path);
            }
        }
    }
    files
}

fn progress_bar(len: usize) -> ProgressBar {
    let pb = ProgressBar::new(len as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.yellow} [{elapsed_precise}] [{bar:40.yellow/black}] {pos}/{len} {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .tick_chars("◐◓◑◒")
            .progress_chars("▰▱ "),
    );
    pb
}

fn ensure_clang_format_exists() -> bool {
    Command::new("clang-format")
        .arg("--version")
        .output()
        .is_ok()
}

fn ensure_clang_tidy_exists() -> bool {
    Command::new("clang-tidy").arg("--version").output().is_ok()
}

fn ensure_clang_format_config() -> Result<()> {
    use std::fs;

    let clang_format_path = Path::new(".clang-format");
    if clang_format_path.exists() {
        return Ok(());
    }

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
    Ok(())
}

fn check_formatting(path: &Path) -> bool {
    let output = Command::new("clang-format")
        .arg("--dry-run")
        .arg("--Werror")
        .arg("-style=file")
        .arg(path)
        .output();

    matches!(output, Ok(out) if !out.status.success() || !out.stderr.is_empty())
}

fn apply_formatting(path: &Path) -> bool {
    let status = Command::new("clang-format")
        .arg("-i")
        .arg("-style=file")
        .arg(path)
        .status();

    matches!(status, Ok(s) if s.success())
}

fn dependency_include_flags(config: &crate::config::CxConfig) -> Vec<String> {
    let mut include_flags = Vec::new();
    if let Some(deps) = &config.dependencies
        && !deps.is_empty()
        && let Ok((paths, cflags, _, _)) = deps::fetch_dependencies(deps)
    {
        for p in paths {
            include_flags.push(format!("-I{}", p.display()));
        }
        include_flags.extend(cflags);
    }
    include_flags
}

fn check_file_with_clang_tidy(
    path: &Path,
    config: &crate::config::CxConfig,
    include_flags: &[String],
    pb: &ProgressBar,
) -> usize {
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
        && let Some(flags) = build_cfg.get_flags()
    {
        cmd.args(flags);
    }
    cmd.args(include_flags);

    let output = cmd.output().ok();
    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let has_issues =
            stdout.contains("warning:") || stdout.contains("error:") || !out.status.success();

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
}

pub fn format_code(check_only: bool) -> Result<()> {
    if !ensure_clang_format_exists() {
        println!("{} clang-format not found.", "x".red());
        println!(
            "   {} Run {} to install it.",
            "💡".yellow(),
            "cx toolchain install".cyan()
        );
        return Ok(());
    }

    ensure_clang_format_config()?;

    let mode_msg = if check_only {
        "Checking formatting..."
    } else {
        "Formatting source code..."
    };
    println!("{} {}", "🎨".magenta(), mode_msg);

    let files = collect_source_files();

    if files.is_empty() {
        println!("{} No source files found to format.", "!".yellow());
        return Ok(());
    }

    let pb = progress_bar(files.len());

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
            if check_formatting(path) {
                unformatted_files.push(path.display().to_string());
            }
        } else if apply_formatting(path) {
            formatted_count += 1;
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
    if !ensure_clang_tidy_exists() {
        println!(
            "{} clang-tidy not found. Please install it first.",
            "x".red()
        );
        return Ok(());
    }

    println!("{} Checking code with clang-tidy...", "🔍".magenta());

    let config = load_config()?;
    let include_flags = dependency_include_flags(&config);
    let files = collect_source_files();
    let pb = progress_bar(files.len());

    let warnings: usize = files
        .par_iter()
        .map(|path| check_file_with_clang_tidy(path, &config, &include_flags, &pb))
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
