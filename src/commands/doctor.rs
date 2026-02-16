//! Doctor command handler
//!
//! Handles `cx doctor`, `cx lock`, and `cx sync` commands.

use anyhow::Result;
use colored::*;
use std::collections::HashMap;

use crate::build;
use crate::config::{CxConfig, Dependency};
use crate::deps;
use crate::lock;
use crate::toolchain;

#[derive(Debug, Default, PartialEq, Eq)]
struct LockComparison {
    missing_in_lock: Vec<String>,
    extra_in_lock: Vec<String>,
    url_mismatch: Vec<(String, String, String)>, // (name, expected, found)
}

impl LockComparison {
    fn is_clean(&self) -> bool {
        self.missing_in_lock.is_empty()
            && self.extra_in_lock.is_empty()
            && self.url_mismatch.is_empty()
    }
}

fn config_git_dependencies(config: &CxConfig) -> HashMap<String, String> {
    let mut deps_map = HashMap::new();
    if let Some(deps) = &config.dependencies {
        for (name, dep) in deps {
            let maybe_url = match dep {
                Dependency::Simple(url) => Some(url.clone()),
                Dependency::Complex { git: Some(url), .. } => Some(url.clone()),
                _ => None, // pkg-config/system deps are not lockfile-pinned
            };
            if let Some(url) = maybe_url {
                deps_map.insert(name.clone(), url);
            }
        }
    }
    deps_map
}

fn collect_expected_dep_diffs(
    git_deps: &HashMap<String, String>,
    lockfile: &lock::LockFile,
) -> (Vec<String>, Vec<(String, String, String)>) {
    let mut missing_in_lock = Vec::new();
    let mut url_mismatch = Vec::new();

    for (name, expected_git) in git_deps {
        match lockfile.get(name) {
            None => missing_in_lock.push(name.clone()),
            Some(entry) if entry.git != *expected_git => {
                url_mismatch.push((name.clone(), expected_git.clone(), entry.git.clone()))
            }
            Some(_) => {}
        }
    }

    (missing_in_lock, url_mismatch)
}

fn collect_extra_lock_entries(
    git_deps: &HashMap<String, String>,
    lockfile: &lock::LockFile,
) -> Vec<String> {
    lockfile
        .packages
        .keys()
        .filter(|name| !git_deps.contains_key(*name))
        .cloned()
        .collect()
}

fn compare_lockfile(config: &CxConfig, lockfile: &lock::LockFile) -> LockComparison {
    let git_deps = config_git_dependencies(config);
    let (mut missing_in_lock, mut url_mismatch) = collect_expected_dep_diffs(&git_deps, lockfile);
    let mut extra_in_lock = collect_extra_lock_entries(&git_deps, lockfile);

    missing_in_lock.sort();
    extra_in_lock.sort();
    url_mismatch.sort_by(|a, b| a.0.cmp(&b.0));

    LockComparison {
        missing_in_lock,
        extra_in_lock,
        url_mismatch,
    }
}

fn print_extra_lock_entry(dep: &str) {
    println!(
        "{} Lockfile contains '{}' which is no longer in cx.toml",
        "x".red(),
        dep
    );
}

fn print_url_mismatch(name: &str, expected: &str, found: &str) {
    println!(
        "{} Dependency '{}' URL mismatch\n  expected: {}\n  lockfile: {}",
        "x".red(),
        name,
        expected,
        found
    );
}

fn print_lock_comparison(comparison: &LockComparison) {
    for dep in &comparison.missing_in_lock {
        println!("{} Dependency '{}' missing from cx.lock", "x".red(), dep);
    }
    for dep in &comparison.extra_in_lock {
        print_extra_lock_entry(dep);
    }
    for (name, expected, found) in &comparison.url_mismatch {
        print_url_mismatch(name, expected, found);
    }
}

fn exit_with_error(message: &str) -> ! {
    eprintln!("{}", message);
    std::process::exit(1);
}

fn load_config_or_exit() -> CxConfig {
    build::load_config()
        .unwrap_or_else(|e| exit_with_error(&format!("Error loading config: {}", e)))
}

fn load_lockfile_or_exit() -> lock::LockFile {
    lock::LockFile::load()
        .unwrap_or_else(|e| exit_with_error(&format!("Error loading lockfile: {}", e)))
}

fn ensure_lockfile_is_clean_or_exit(config: &CxConfig, lockfile: &lock::LockFile, message: &str) {
    let comparison = compare_lockfile(config, lockfile);
    if comparison.is_clean() {
        return;
    }
    print_lock_comparison(&comparison);
    exit_with_error(message);
}

fn refresh_empty_lockfile_or_exit() {
    let empty = lock::LockFile::default();
    if let Err(e) = empty.save() {
        exit_with_error(&format!("Error writing lockfile: {}", e));
    }
    println!("{} Lockfile refreshed (no git dependencies).", "✓".green());
}

fn update_dependencies_or_exit() {
    if let Err(e) = deps::update_dependencies() {
        exit_with_error(&format!("Error updating dependencies: {}", e));
    }
}

fn run_lock_check() {
    println!("{} Verifying lockfile...", "🔒".blue());
    let lockfile = load_lockfile_or_exit();
    let config = load_config_or_exit();
    ensure_lockfile_is_clean_or_exit(
        &config,
        &lockfile,
        &format!(
            "{} Lockfile out of sync. Run 'cx lock --update'.",
            "x".red()
        ),
    );
    println!("{} Lockfile is in sync.", "✓".green());
}

fn run_lock_update() {
    println!("{} Updating lockfile...", "🔄".blue());
    let config = load_config_or_exit();

    if config_git_dependencies(&config).is_empty() {
        refresh_empty_lockfile_or_exit();
        return;
    }

    update_dependencies_or_exit();
    let lockfile = lock::LockFile::load()
        .unwrap_or_else(|e| exit_with_error(&format!("Error loading updated lockfile: {}", e)));

    ensure_lockfile_is_clean_or_exit(
        &config,
        &lockfile,
        &format!("{} Lockfile update incomplete.", "x".red()),
    );
    println!("{} Lockfile updated.", "✓".green());
}

fn sync_with_lockfile_or_exit(config: &CxConfig) {
    let lockfile = load_lockfile_or_exit();
    ensure_lockfile_is_clean_or_exit(
        config,
        &lockfile,
        &format!(
            "{} Refusing to sync: lockfile is out of sync. Run 'cx lock --update' first.",
            "x".red()
        ),
    );
}

fn fetch_dependencies_for_sync_or_exit(config: CxConfig) {
    let Some(deps) = config.dependencies else {
        println!("No dependencies found in cx.toml.");
        return;
    };

    if deps.is_empty() {
        println!("No dependencies found in cx.toml.");
        return;
    }

    match deps::fetch_dependencies(&deps) {
        Ok(_) => println!("{} Dependencies synchronized.", "✓".green()),
        Err(e) => exit_with_error(&format!("Error synchronizing: {}", e)),
    }
}

/// Run the `cx doctor` command to diagnose system issues
pub fn run_doctor() -> Result<()> {
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

/// Handle the `cx lock` command for managing lockfiles
pub fn handle_lock(update: bool, check: bool) {
    if check {
        run_lock_check();
        return;
    }

    if update {
        run_lock_update();
        return;
    }

    println!("Use --check to verify or --update to update/regenerate.");
}

/// Handle the `cx sync` command for synchronizing dependencies
pub fn handle_sync() {
    println!(
        "{} Synchronizing dependencies with lockfile...",
        "📦".blue()
    );

    let config = load_config_or_exit();
    sync_with_lockfile_or_exit(&config);
    fetch_dependencies_for_sync_or_exit(config);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BuildConfig, PackageConfig};

    fn test_config_with_git_dep(name: &str, url: &str) -> CxConfig {
        let mut deps = HashMap::new();
        deps.insert(name.to_string(), Dependency::Simple(url.to_string()));
        CxConfig {
            package: PackageConfig {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                edition: "c++20".to_string(),
            },
            dependencies: Some(deps),
            build: Some(BuildConfig::default()),
            ..Default::default()
        }
    }

    #[test]
    fn compare_lockfile_detects_missing() {
        let config = test_config_with_git_dep("fmt", "https://github.com/fmtlib/fmt.git");
        let lockfile = lock::LockFile::default();
        let cmp = compare_lockfile(&config, &lockfile);
        assert_eq!(cmp.missing_in_lock, vec!["fmt".to_string()]);
    }

    #[test]
    fn compare_lockfile_detects_extra() {
        let config = CxConfig::default();
        let mut lockfile = lock::LockFile::default();
        lockfile.insert(
            "fmt".to_string(),
            "https://github.com/fmtlib/fmt.git".to_string(),
            "abc123".to_string(),
        );
        let cmp = compare_lockfile(&config, &lockfile);
        assert_eq!(cmp.extra_in_lock, vec!["fmt".to_string()]);
    }

    #[test]
    fn compare_lockfile_detects_url_mismatch() {
        let config = test_config_with_git_dep("fmt", "https://github.com/fmtlib/fmt.git");
        let mut lockfile = lock::LockFile::default();
        lockfile.insert(
            "fmt".to_string(),
            "https://github.com/fmtlib/fmt-mirror.git".to_string(),
            "abc123".to_string(),
        );
        let cmp = compare_lockfile(&config, &lockfile);
        assert_eq!(cmp.url_mismatch.len(), 1);
        assert_eq!(cmp.url_mismatch[0].0, "fmt");
    }
}
