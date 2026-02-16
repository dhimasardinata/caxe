//! Dependency fetching and build logic.
//!
//! This module handles downloading, building, and caching dependencies from Git.
//!
//! ## Features
//!
//! - Git clone with tag/branch/rev pinning
//! - Custom build commands per dependency
//! - SHA256 hash verification for prebuilt binaries
//! - Global cache at `~/.cx/cache`

use crate::config::Dependency;
use anyhow::{Context, Result};
use colored::*;

use git2::Repository;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, copy};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Verify a file's SHA256 hash against an expected value.
/// Returns Ok(true) if hash matches, Ok(false) if no expected hash,
/// or Err if file can't be read or hash doesn't match.
#[allow(dead_code)]
pub fn verify_sha256(path: &Path, expected_hash: Option<&str>) -> Result<bool> {
    let expected = match expected_hash {
        Some(h) => h,
        None => return Ok(true), // No hash to verify, consider it valid
    };

    let mut file = fs::File::open(path).with_context(|| {
        format!(
            "Failed to open file for hash verification: {}",
            path.display()
        )
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let result = hasher.finalize();
    let actual_hash = format!("{:x}", result);

    if actual_hash.eq_ignore_ascii_case(expected) {
        Ok(true)
    } else {
        Err(anyhow::anyhow!(
            "SHA256 hash mismatch for {}:\n  Expected: {}\n  Actual:   {}",
            path.display(),
            expected,
            actual_hash
        ))
    }
}

/// Known library configurations for prebuilt binary downloads
struct PrebuiltConfig {
    /// GitHub release asset pattern (rust format string with {version})
    asset_pattern: &'static str,
    /// Path inside the zip where the lib file is located
    lib_path: &'static str,
    /// Include path inside the zip
    include_path: &'static str,
}

/// Get prebuilt config for known libraries
fn get_prebuilt_config(name: &str) -> Option<PrebuiltConfig> {
    match name.to_lowercase().as_str() {
        "glfw" => Some(PrebuiltConfig {
            asset_pattern: "glfw-{version}.bin.WIN64.zip",
            // lib-static-ucrt is compatible with dynamic CRT (/MD)
            lib_path: "glfw-{version}.bin.WIN64/lib-static-ucrt/glfw3.lib",
            include_path: "glfw-{version}.bin.WIN64/include",
        }),
        "sdl2" | "sdl" => Some(PrebuiltConfig {
            asset_pattern: "SDL2-devel-{version}-VC.zip",
            lib_path: "SDL2-{version}/lib/x64/SDL2.lib",
            include_path: "SDL2-{version}/include",
        }),
        _ => None,
    }
}

/// Detect MSVC version from compiler path to select compatible prebuilt lib
/// Returns the lib folder suffix (e.g., "lib-vc2022", "lib-vc2019")
fn detect_msvc_lib_folder() -> Option<&'static str> {
    // Try to detect MSVC version from environment or vswhere
    // MSVC version mapping:
    // - 19.30+ = VS 2022 (lib-vc2022)
    // - 19.20+ = VS 2019 (lib-vc2019)
    // - 19.10+ = VS 2017 (lib-vc2017)
    // - 19.00+ = VS 2015 (lib-vc2015)

    // Check VS version from vswhere or environment
    #[cfg(windows)]
    {
        // Try to find cl.exe and get its version
        if let Ok(output) = Command::new("cl.exe").output() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Parse version from "Microsoft (R) C/C++ Optimizing Compiler Version 19.XX.XXXXX"
            // Note: MSVC 19.50+ (VS 2022 17.14+) is too new for prebuilt libs, skip prebuilt
            if stderr.contains("Version 19.5") || stderr.contains("Version 19.4") {
                // VS 2022 17.10+ - too new, prebuilt libs have CRT mismatch
                return None;
            } else if stderr.contains("Version 19.3") {
                return Some("lib-vc2022");
            } else if stderr.contains("Version 19.2") {
                return Some("lib-vc2019");
            } else if stderr.contains("Version 19.1") {
                return Some("lib-vc2017");
            } else if stderr.contains("Version 19.0") {
                return Some("lib-vc2015");
            }
        }

        // Fallback: try to detect from VS install path (check both x64 and x86 Program Files)
        // Note: VS 2022 prebuilt libs have CRT mismatch issues with newer VS updates, so skip
        if std::path::Path::new("C:\\Program Files\\Microsoft Visual Studio\\2022").exists()
            || std::path::Path::new("C:\\Program Files (x86)\\Microsoft Visual Studio\\2022")
                .exists()
        {
            // VS 2022 has CRT compatibility issues with prebuilt libs, use source build
            return None;
        } else if std::path::Path::new("C:\\Program Files (x86)\\Microsoft Visual Studio\\2019")
            .exists()
        {
            return Some("lib-vc2019");
        } else if std::path::Path::new("C:\\Program Files (x86)\\Microsoft Visual Studio\\2017")
            .exists()
        {
            return Some("lib-vc2017");
        }
    }

    None
}

/// Try to download prebuilt binaries from GitHub releases
/// Returns Ok(true) if prebuilt was downloaded, Ok(false) if not available
fn try_download_prebuilt(
    name: &str,
    url: &str,
    tag: Option<&str>,
    lib_path: &Path,
    output_file: &str,
) -> Result<bool> {
    // Only works on Windows for now
    #[cfg(not(windows))]
    {
        return Ok(false);
    }

    // Need a tag/version to find the right release
    let version = match tag {
        Some(t) => t.trim_start_matches('v').trim_start_matches("release-"),
        None => return Ok(false),
    };

    // Get prebuilt config for this library
    let config = match get_prebuilt_config(name) {
        Some(c) => c,
        None => return Ok(false),
    };

    // Parse GitHub owner/repo from URL
    let (owner, repo) = match parse_github_url(url) {
        Some(pair) => pair,
        None => return Ok(false),
    };

    // Build release URL
    let asset_name = config.asset_pattern.replace("{version}", version);
    let download_url = format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        owner,
        repo,
        tag.unwrap_or(version),
        asset_name
    );

    // Check if output already exists
    let expected_output = lib_path.join(output_file);
    if expected_output.exists() {
        return Ok(true);
    }

    println!("   {} Checking for prebuilt {}...", "⚡".cyan(), name);

    // Try to download
    let agent = ureq::agent();
    let response = match agent.get(&download_url).call() {
        Ok(r) => r,
        Err(_) => {
            // No prebuilt available, fall back to source build
            return Ok(false);
        }
    };

    if response.status() != 200 {
        return Ok(false);
    }

    println!(
        "   {} Downloading prebuilt {} (faster!)...",
        "📦".blue(),
        name
    );

    // Download to temp file
    let temp_zip = lib_path.join("_prebuilt.zip");
    let mut file = fs::File::create(&temp_zip)?;
    let body = response.into_body();
    let mut reader = body.into_reader();
    copy(&mut reader, &mut file)?;
    drop(file);

    // Extract zip
    let zip_file = fs::File::open(&temp_zip)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;

    // Extract lib file - search by suffix since path format may vary
    let lib_suffix = config
        .lib_path
        .replace("{version}", version)
        .split('/')
        .next_back()
        .unwrap_or("glfw3.lib")
        .to_string();

    // Detect MSVC version for CRT-compatible lib selection
    let msvc_lib_folder = detect_msvc_lib_folder();

    let mut lib_found = false;
    for i in 0..archive.len() {
        if let Ok(mut entry) = archive.by_index(i) {
            let entry_name = entry.name().to_string();

            // Check if this is the target lib file
            if !entry_name.ends_with(&lib_suffix) || entry_name.contains("_mt.") {
                continue;
            }

            // Only use prebuilt if we detected a compatible MSVC version
            // None means VS 2022+ which has CRT mismatch issues
            let is_preferred = if let Some(lib_folder) = msvc_lib_folder {
                entry_name.contains(lib_folder)
            } else {
                // No compatible lib folder detected, skip prebuilt
                false
            };

            if is_preferred {
                let out_path = lib_path.join(output_file);
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out_file = fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out_file)?;
                lib_found = true;
                break;
            }
        }
    }

    if !lib_found {
        // Cleanup and fallback to source build
        let _ = fs::remove_file(&temp_zip);
        return Ok(false);
    }

    // Extract includes
    let include_prefix = config.include_path.replace("{version}", version);
    for i in 0..archive.len() {
        if let Ok(mut entry) = archive.by_index(i) {
            let entry_name = entry.name().to_string();
            if entry_name.starts_with(&include_prefix) && !entry.is_dir() {
                let relative = entry_name
                    .strip_prefix(&include_prefix)
                    .unwrap_or(&entry_name);
                let out_path = lib_path
                    .join("include")
                    .join(relative.trim_start_matches('/'));
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out_file = fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out_file)?;
            }
        }
    }

    // Cleanup
    let _ = fs::remove_file(&temp_zip);

    println!("   {} Prebuilt {} ready!", "✓".green(), name);

    Ok(true)
}

/// Parse GitHub URL to get owner/repo
fn parse_github_url(url: &str) -> Option<(String, String)> {
    // Handle: https://github.com/owner/repo.git
    let url = url.trim_end_matches(".git");
    if url.contains("github.com") {
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 2 {
            let repo = parts.last()?;
            let owner = parts.get(parts.len() - 2)?;
            return Some((owner.to_string(), repo.to_string()));
        }
    }
    None
}

/// Module file info: (module_source_path, dependency_root_path)
pub type ModuleFile = (PathBuf, PathBuf);

pub type FetchResult = (Vec<PathBuf>, Vec<String>, Vec<String>, Vec<ModuleFile>);

#[derive(Clone, Copy, Debug)]
pub struct FetchOptions {
    pub enforce_lock: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self { enforce_lock: true }
    }
}

pub fn fetch_dependencies(deps: &HashMap<String, Dependency>) -> Result<FetchResult> {
    fetch_dependencies_with_options(deps, FetchOptions::default())
}

pub fn fetch_dependencies_with_options(
    deps: &HashMap<String, Dependency>,
    options: FetchOptions,
) -> Result<FetchResult> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");
    fs::create_dir_all(&cache_dir)?;

    let mut lockfile = crate::lock::LockFile::load().unwrap_or_default();
    let mut state = FetchState::default();

    if !deps.is_empty() {
        println!("{} Checking {} dependencies...", "📦".blue(), deps.len());
    }

    for (name, dep_data) in deps {
        if let Some(pkg_name) = pkg_dependency_name(dep_data) {
            resolve_system_package(pkg_name, &mut state);
            continue;
        }

        let Some(spec) = extract_git_dependency_spec(dep_data) else {
            continue;
        };
        process_git_dependency(name, &spec, &cache_dir, options, &mut lockfile, &mut state)?;
    }

    lockfile
        .packages
        .retain(|name, _| state.expected_git_deps.contains(name));
    lockfile.save()?;
    Ok(state.into_result())
}

#[derive(Default)]
struct FetchState {
    expected_git_deps: HashSet<String>,
    include_paths: Vec<PathBuf>,
    include_seen: HashSet<PathBuf>,
    extra_cflags: Vec<String>,
    link_flags: Vec<String>,
    module_files: Vec<ModuleFile>,
    module_seen: HashSet<PathBuf>,
}

impl FetchState {
    fn add_include(&mut self, candidate: PathBuf) {
        if candidate.exists() && self.include_seen.insert(candidate.clone()) {
            self.include_paths.push(candidate);
        }
    }

    fn into_result(self) -> FetchResult {
        (
            self.include_paths,
            self.extra_cflags,
            self.link_flags,
            self.module_files,
        )
    }
}

#[derive(Clone)]
struct GitDependencySpec {
    url: String,
    build_script: Option<String>,
    output_file: Option<String>,
    tag: Option<String>,
    branch: Option<String>,
    rev: Option<String>,
}

fn pkg_dependency_name(dep_data: &Dependency) -> Option<&str> {
    if let Dependency::Complex {
        pkg: Some(pkg_name),
        ..
    } = dep_data
    {
        Some(pkg_name)
    } else {
        None
    }
}

fn extract_git_dependency_spec(dep_data: &Dependency) -> Option<GitDependencySpec> {
    match dep_data {
        Dependency::Simple(url) => Some(GitDependencySpec {
            url: url.clone(),
            build_script: None,
            output_file: None,
            tag: None,
            branch: None,
            rev: None,
        }),
        Dependency::Complex {
            git: Some(url),
            build,
            output,
            tag,
            branch,
            rev,
            ..
        } => Some(GitDependencySpec {
            url: url.clone(),
            build_script: build.clone(),
            output_file: output.clone(),
            tag: tag.clone(),
            branch: branch.clone(),
            rev: rev.clone(),
        }),
        _ => None,
    }
}

fn resolve_system_package(pkg_name: &str, state: &mut FetchState) {
    println!("   {} Resolving system pkg: {}", "🔎".cyan(), pkg_name);

    let cflags_ok = append_pkg_config_flags(pkg_name, "--cflags", &mut state.extra_cflags);
    if !cflags_ok {
        println!("{} Warning: pkg-config tool not found", "!".yellow());
        return;
    }

    let libs_ok = append_pkg_config_flags(pkg_name, "--libs", &mut state.link_flags);
    if !libs_ok {
        println!("{} Package '{}' not found via pkg-config", "x".red(), pkg_name);
    }
}

fn append_pkg_config_flags(pkg_name: &str, flag_kind: &str, out: &mut Vec<String>) -> bool {
    let Ok(output) = Command::new("pkg-config").args([flag_kind, pkg_name]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !out_str.is_empty() {
        out.extend(out_str.split_whitespace().map(ToOwned::to_owned));
    }
    true
}

fn process_git_dependency(
    name: &str,
    spec: &GitDependencySpec,
    cache_dir: &Path,
    options: FetchOptions,
    lockfile: &mut crate::lock::LockFile,
    state: &mut FetchState,
) -> Result<()> {
    state.expected_git_deps.insert(name.to_string());

    let (lib_path, is_vendor) = resolve_dependency_path(name, cache_dir)?;
    let repo = open_or_clone_repo(name, &spec.url, &lib_path, is_vendor)?;

    let locked_commit = locked_commit_for(lockfile, name, &spec.url, options.enforce_lock);
    if let Some((oid, checkout_msg)) =
        select_checkout_target(&repo, spec, locked_commit.as_deref())
    {
        checkout_repo_target(&repo, oid, &checkout_msg)?;
    }

    refresh_lockfile_entry(&repo, lockfile, name, &spec.url);
    maybe_build_dependency(name, spec, &lib_path)?;
    register_include_paths(&lib_path, state);
    collect_module_files(&lib_path, state);
    collect_link_outputs(&lib_path, spec.output_file.as_deref(), &mut state.link_flags);
    Ok(())
}

fn resolve_dependency_path(name: &str, cache_dir: &Path) -> Result<(PathBuf, bool)> {
    let vendor_path = std::env::current_dir()?.join("vendor").join(name);
    if vendor_path.exists() {
        Ok((vendor_path, true))
    } else {
        Ok((cache_dir.join(name), false))
    }
}

fn open_or_clone_repo(name: &str, url: &str, lib_path: &Path, is_vendor: bool) -> Result<Repository> {
    if lib_path.exists() {
        if is_vendor {
            println!("   {} Using vendor: {}", "📦".blue(), name);
        } else {
            println!("   {} Using cached: {}", "⚡".green(), name);
        }
        return Repository::open(lib_path)
            .with_context(|| format!("Failed to open cached dependency '{}'", name));
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷"),
    );
    pb.set_message(format!("Downloading {}...", name));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    match Repository::clone(url, lib_path) {
        Ok(repo) => {
            pb.finish_with_message(format!("{} Downloaded {}", "✓".green(), name));
            Ok(repo)
        }
        Err(err) => {
            pb.finish_with_message(format!("{} Failed {}", "x".red(), name));
            Err(anyhow::anyhow!(
                "Failed to clone dependency '{}': {}",
                name,
                err
            ))
        }
    }
}

fn locked_commit_for(
    lockfile: &crate::lock::LockFile,
    name: &str,
    url: &str,
    enforce_lock: bool,
) -> Option<String> {
    if !enforce_lock {
        return None;
    }
    lockfile
        .get(name)
        .filter(|entry| entry.git == url)
        .map(|entry| entry.rev.clone())
}

fn select_checkout_target(
    repo: &Repository,
    spec: &GitDependencySpec,
    locked_commit: Option<&str>,
) -> Option<(git2::Oid, String)> {
    if let Some(rev) = spec.rev.as_deref()
        && let Ok(oid) = git2::Oid::from_str(rev)
        && repo.find_object(oid, None).is_ok()
    {
        return Some((oid, format!("commit {}", short_hash(rev))));
    }

    if let Some(tag) = spec.tag.as_deref() {
        let refname = format!("refs/tags/{}", tag);
        if let Ok(reference) = repo.find_reference(&refname)
            && let Ok(commit) = reference.peel_to_commit()
        {
            return Some((commit.id(), format!("tag {}", tag)));
        }
    }

    if let Some(branch) = spec.branch.as_deref()
        && let Some(branch_oid) = find_branch_commit(repo, branch)
    {
        return Some((branch_oid, format!("branch {}", branch)));
    }

    if let Some(rev) = locked_commit
        && let Ok(oid) = git2::Oid::from_str(rev)
        && repo.find_object(oid, None).is_ok()
    {
        return Some((oid, format!("locked {}", short_hash(rev))));
    }

    None
}

fn find_branch_commit(repo: &Repository, branch: &str) -> Option<git2::Oid> {
    if let Ok(reference) = repo.find_branch(branch, git2::BranchType::Local)
        && let Ok(commit) = reference.get().peel_to_commit()
    {
        return Some(commit.id());
    }

    let remote_ref = format!("origin/{}", branch);
    if let Ok(reference) = repo.find_branch(&remote_ref, git2::BranchType::Remote)
        && let Ok(commit) = reference.get().peel_to_commit()
    {
        return Some(commit.id());
    }

    None
}

fn short_hash(rev: &str) -> &str {
    if rev.len() > 7 {
        &rev[..7]
    } else {
        rev
    }
}

fn checkout_repo_target(repo: &Repository, oid: git2::Oid, checkout_msg: &str) -> Result<()> {
    repo.set_head_detached(oid)?;
    let obj = repo.find_object(oid, None)?;
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_tree(&obj, Some(&mut checkout_opts))
        .context(format!("Failed to checkout {}", checkout_msg))?;
    println!("   {} Locked to {}", "📌".blue(), checkout_msg);
    Ok(())
}

fn refresh_lockfile_entry(
    repo: &Repository,
    lockfile: &mut crate::lock::LockFile,
    name: &str,
    url: &str,
) {
    if let Ok(head) = repo.head()
        && let Ok(target) = head.peel_to_commit()
    {
        lockfile.insert(name.to_string(), url.to_string(), target.id().to_string());
    }
}

fn maybe_build_dependency(name: &str, spec: &GitDependencySpec, lib_path: &Path) -> Result<()> {
    let output_name = spec.output_file.as_deref().unwrap_or("");
    let prebuilt_success = if output_name.is_empty() {
        false
    } else {
        try_download_prebuilt(name, &spec.url, spec.tag.as_deref(), lib_path, output_name)
            .unwrap_or(false)
    };

    if prebuilt_success {
        return Ok(());
    }
    let Some(cmd_str) = spec.build_script.as_deref() else {
        return Ok(());
    };

    let should_build = if output_name.is_empty() {
        true
    } else {
        !lib_path.join(output_name).exists()
    };
    if !should_build {
        return Ok(());
    }

    println!("   {} Building {}...", "🔨".yellow(), name);
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd_str])
            .current_dir(lib_path)
            .status()
    } else {
        Command::new("sh")
            .args(["-c", cmd_str])
            .current_dir(lib_path)
            .status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => {
            println!("{} Build script failed for {}", "x".red(), name);
            Ok(())
        }
    }
}

fn register_include_paths(lib_path: &Path, state: &mut FetchState) {
    state.add_include(lib_path.to_path_buf());
    state.add_include(lib_path.join("include"));
    state.add_include(lib_path.join("src"));
    state.add_include(lib_path.join("build").join("include"));
    state.add_include(lib_path.join("build").join("include").join("SDL2"));
    state.add_include(lib_path.join("dist"));
    state.add_include(lib_path.join("dist").join("include"));
}

fn collect_module_files(lib_path: &Path, state: &mut FetchState) {
    let src_path = lib_path.join("src");
    if !src_path.exists() {
        return;
    }
    let Ok(entries) = fs::read_dir(&src_path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && ["cppm", "ixx", "mpp"].contains(&ext)
            && state.module_seen.insert(path.clone())
        {
            state.module_files.push((path, lib_path.to_path_buf()));
        }
    }
}

fn collect_link_outputs(lib_path: &Path, output_file: Option<&str>, link_flags: &mut Vec<String>) {
    let Some(out_file) = output_file else {
        return;
    };

    for single_output in out_file.split(',').map(|s| s.trim()) {
        let full_lib_path = lib_path.join(single_output);
        if full_lib_path.exists() {
            link_flags.push(full_lib_path.to_string_lossy().to_string());
        } else {
            println!(
                "{} Warning: Output file not found: {}",
                "!".yellow(),
                full_lib_path.display()
            );
        }
    }
}
