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
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
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
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    file.write_all(&buffer)?;
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

pub fn fetch_dependencies(
    deps: &HashMap<String, Dependency>,
) -> Result<(Vec<PathBuf>, Vec<String>, Vec<String>)> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");
    fs::create_dir_all(&cache_dir)?;

    let mut lockfile = crate::lock::LockFile::load().unwrap_or_default();

    let mut include_paths = Vec::new(); // Pure paths for -I or /I
    let mut extra_cflags = Vec::new(); // pkg-config flags
    let mut link_flags = Vec::new();

    if !deps.is_empty() {
        println!("{} Checking {} dependencies...", "📦".blue(), deps.len());
    }

    for (name, dep_data) in deps {
        // --- CASE 1: System Package (pkg-config) ---
        if let Dependency::Complex {
            pkg: Some(pkg_name),
            ..
        } = dep_data
        {
            println!("   {} Resolving system pkg: {}", "🔎".cyan(), pkg_name);

            // 1. Get CFLAGS (Include paths)
            match Command::new("pkg-config")
                .args(["--cflags", pkg_name])
                .output()
            {
                Ok(out) => {
                    let out_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !out_str.is_empty() {
                        for flag in out_str.split_whitespace() {
                            extra_cflags.push(flag.to_string());
                        }
                    }
                }
                Err(_) => println!("{} Warning: pkg-config tool not found", "!".yellow()),
            }

            // 2. Get LIBS (Link paths)
            if let Ok(out) = Command::new("pkg-config")
                .args(["--libs", pkg_name])
                .output()
            {
                if !out.status.success() {
                    println!(
                        "{} Package '{}' not found via pkg-config",
                        "x".red(),
                        pkg_name
                    );
                }
                let out_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !out_str.is_empty() {
                    for flag in out_str.split_whitespace() {
                        link_flags.push(flag.to_string());
                    }
                }
            }
            continue;
        }

        // --- CASE 2: Git Dependency ---
        let (url, build_script, output_file, tag, branch, rev) = match dep_data {
            Dependency::Simple(u) => (u.clone(), None, None, None, None, None),
            Dependency::Complex {
                git: Some(u),
                build,
                output,
                tag,
                branch,
                rev,
                ..
            } => (
                u.clone(),
                build.clone(),
                output.clone(),
                tag.clone(),
                branch.clone(),
                rev.clone(),
            ),
            _ => continue,
        };

        // Check for local vendor override
        let vendor_path = std::env::current_dir()?.join("vendor").join(name);

        let (lib_path, is_vendor) = if vendor_path.exists() {
            (vendor_path, true)
        } else {
            (cache_dir.join(name), false)
        };

        // A. Download (Clone) or Open Existing
        let repo = if !lib_path.exists() {
            // Cannot download if we expected vendor but it's missing (should have fallen back to cache)
            // Logic: If vendor exists, use it. If not, use cache.
            // If cache missing, download to cache.

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.blue} {msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner())
                    .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷"),
            );
            pb.set_message(format!("Downloading {}...", name));
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            match Repository::clone(&url, &lib_path) {
                Ok(r) => {
                    pb.finish_with_message(format!("{} Downloaded {}", "✓".green(), name));
                    r
                }
                Err(e) => {
                    pb.finish_with_message(format!("{} Failed {}", "x".red(), name));
                    println!("Error: {}", e);
                    continue;
                }
            }
        } else {
            if is_vendor {
                println!("   {} Using vendor: {}", "📦".blue(), name);
            } else {
                println!("   {} Using cached: {}", "⚡".green(), name);
            }
            match Repository::open(&lib_path) {
                Ok(r) => r,
                Err(_) => continue,
            }
        };

        // B. Pinning / Checkout Logic (v0.1.5 + v0.1.8 Lockfile)
        let mut obj_to_checkout = None;
        let mut checkout_msg = String::new();

        // Lockfile Check
        let mut locked_commit = None;
        if let Some(lock_entry) = lockfile.get(name)
            && lock_entry.git == url
        {
            locked_commit = Some(lock_entry.rev.clone());
        }

        if let Some(r) = rev {
            // 1. Explicit Config Commit (Highest Priority)
            if let Ok(oid) = git2::Oid::from_str(&r)
                && let Ok(obj) = repo.find_object(oid, None)
            {
                obj_to_checkout = Some(obj);
                checkout_msg = format!("commit {}", &r[..7]);
            }
        } else if let Some(ref t) = tag {
            // 2. Explicit Tag
            let refname = format!("refs/tags/{}", t);
            if let Ok(r_ref) = repo.find_reference(&refname)
                && let Ok(obj) = r_ref.peel_to_commit()
            {
                obj_to_checkout = Some(obj.into_object());
                checkout_msg = format!("tag {}", t);
            }
        } else if let Some(b) = branch {
            // 3. Explicit Branch
            if let Ok(r_ref) = repo.find_branch(&b, git2::BranchType::Local) {
                if let Ok(obj) = r_ref.get().peel_to_commit() {
                    obj_to_checkout = Some(obj.into_object());
                    checkout_msg = format!("branch {}", b);
                }
            } else {
                let remote_ref = format!("origin/{}", b);
                if let Ok(r_ref) = repo.find_branch(&remote_ref, git2::BranchType::Remote)
                    && let Ok(obj) = r_ref.get().peel_to_commit()
                {
                    obj_to_checkout = Some(obj.into_object());
                    checkout_msg = format!("branch {}", b);
                }
            }
        } else if let Some(rev) = locked_commit {
            // 4. Lockfile Commit (Zero Config Reproducibility)
            if let Ok(oid) = git2::Oid::from_str(&rev)
                && let Ok(obj) = repo.find_object(oid, None)
            {
                obj_to_checkout = Some(obj);
                checkout_msg = format!("locked {}", &rev[..7]);
            }
        }

        if let Some(obj) = obj_to_checkout {
            repo.set_head_detached(obj.id())?;
            let mut checkout_opts = git2::build::CheckoutBuilder::new();
            checkout_opts.force();
            repo.checkout_tree(&obj, Some(&mut checkout_opts))
                .context(format!("Failed to checkout {}", checkout_msg))?;
            println!("   {} Locked to {}", "📌".blue(), checkout_msg);
        }

        // Update Lockfile with current HEAD
        if let Ok(head) = repo.head()
            && let Ok(target) = head.peel_to_commit()
        {
            let current_hash = target.id().to_string();
            lockfile.insert(name.clone(), url.clone(), current_hash);
        }

        // C. Try Prebuilt Binary (Skip slow source build!)
        let tag_ref = tag.as_deref();
        let out_filename = output_file.as_deref().unwrap_or("");

        // Try prebuilt first (for known libraries like GLFW, SDL2)
        let prebuilt_success = if !out_filename.is_empty() {
            try_download_prebuilt(name, &url, tag_ref, &lib_path, out_filename).unwrap_or(false)
        } else {
            false
        };

        // D. Build Custom Script (If prebuilt failed and script exists)
        if !prebuilt_success && let Some(cmd_str) = build_script {
            let should_build = if !out_filename.is_empty() {
                !lib_path.join(out_filename).exists()
            } else {
                true
            };

            if should_build {
                println!("   {} Building {}...", "🔨".yellow(), name);
                let status = if cfg!(target_os = "windows") {
                    Command::new("cmd")
                        .args(["/C", &cmd_str])
                        .current_dir(&lib_path)
                        .status()
                } else {
                    Command::new("sh")
                        .args(["-c", &cmd_str])
                        .current_dir(&lib_path)
                        .status()
                };

                match status {
                    Ok(s) if s.success() => {}
                    _ => {
                        println!("{} Build script failed for {}", "x".red(), name);
                        continue;
                    }
                }
            }
        }

        // D. Register Includes Flags (Return Paths)
        include_paths.push(lib_path.clone());
        include_paths.push(lib_path.join("include"));
        include_paths.push(lib_path.join("src"));
        // CMake-built dependencies often generate headers in the build directory
        include_paths.push(lib_path.join("build").join("include"));
        include_paths.push(lib_path.join("build").join("include").join("SDL2"));
        // GLAD 2.0 outputs to dist/ directory
        include_paths.push(lib_path.join("dist"));
        include_paths.push(lib_path.join("dist").join("include"));

        // E. Smart Linking Logic (Zero Config Header-Only Support)
        if let Some(out_file) = output_file {
            // Support comma-separated output files
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
    }

    lockfile.save()?;
    Ok((include_paths, extra_cflags, link_flags))
}
