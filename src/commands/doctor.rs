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
        println!("{} Verifying lockfile...", "🔒".blue());
        match lock::LockFile::load() {
            Ok(lockfile) => match build::load_config() {
                Ok(config) => {
                    let comparison = compare_lockfile(&config, &lockfile);
                    if comparison.is_clean() {
                        println!("{} Lockfile is in sync.", "✓".green());
                    } else {
                        print_lock_comparison(&comparison);
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
        let config = match build::load_config() {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Error loading config: {}", e);
                std::process::exit(1);
            }
        };

        let has_git_deps = !config_git_dependencies(&config).is_empty();
        if !has_git_deps {
            let empty = lock::LockFile::default();
            if let Err(e) = empty.save() {
                eprintln!("Error writing lockfile: {}", e);
                std::process::exit(1);
            }
            println!("{} Lockfile refreshed (no git dependencies).", "✓".green());
            return;
        }

        if let Err(e) = deps::update_dependencies() {
            eprintln!("Error updating dependencies: {}", e);
            std::process::exit(1);
        }

        match lock::LockFile::load() {
            Ok(lockfile) => {
                let comparison = compare_lockfile(&config, &lockfile);
                if comparison.is_clean() {
                    println!("{} Lockfile updated.", "✓".green());
                } else {
                    print_lock_comparison(&comparison);
                    eprintln!("{} Lockfile update incomplete.", "x".red());
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Error loading updated lockfile: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("Use --check to verify or --update to update/regenerate.");
    }
}

/// Handle the `cx sync` command for synchronizing dependencies
pub fn handle_sync() {
    println!(
        "{} Synchronizing dependencies with lockfile...",
        "📦".blue()
    );

    let config = match build::load_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    let lockfile = match lock::LockFile::load() {
        Ok(lockfile) => lockfile,
        Err(e) => {
            eprintln!("Error loading lockfile: {}", e);
            std::process::exit(1);
        }
    };

    let comparison = compare_lockfile(&config, &lockfile);
    if !comparison.is_clean() {
        print_lock_comparison(&comparison);
        eprintln!(
            "{} Refusing to sync: lockfile is out of sync. Run 'cx lock --update' first.",
            "x".red()
        );
        std::process::exit(1);
    }

    if let Some(deps) = config.dependencies {
        if deps.is_empty() {
            println!("No dependencies found in cx.toml.");
            return;
        }
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
