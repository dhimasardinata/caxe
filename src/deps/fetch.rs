use crate::config::Dependency;
use anyhow::{Context, Result};
use colored::*;
use dirs;
use git2::Repository;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

pub fn fetch_dependencies(
    deps: &HashMap<String, Dependency>,
) -> Result<(Vec<String>, Vec<String>)> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");
    fs::create_dir_all(&cache_dir)?;

    let mut lockfile = crate::lock::LockFile::load().unwrap_or_default();

    let mut include_flags = Vec::new();
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
                .args(&["--cflags", pkg_name])
                .output()
            {
                Ok(out) => {
                    let out_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !out_str.is_empty() {
                        for flag in out_str.split_whitespace() {
                            include_flags.push(flag.to_string());
                        }
                    }
                }
                Err(_) => println!("{} Warning: pkg-config tool not found", "!".yellow()),
            }

            // 2. Get LIBS (Link paths)
            match Command::new("pkg-config")
                .args(&["--libs", pkg_name])
                .output()
            {
                Ok(out) => {
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
                Err(_) => {}
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

        let lib_path = cache_dir.join(name);

        // A. Download (Clone) or Open Existing
        let repo = if !lib_path.exists() {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
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
            println!("   {} Using cached: {}", "⚡".green(), name);
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
        if let Some(lock_entry) = lockfile.get(name) {
            if lock_entry.git == url {
                locked_commit = Some(lock_entry.rev.clone());
            }
        }

        if let Some(r) = rev {
            // 1. Explicit Config Commit (Highest Priority)
            if let Ok(oid) = git2::Oid::from_str(&r) {
                if let Ok(obj) = repo.find_object(oid, None) {
                    obj_to_checkout = Some(obj);
                    checkout_msg = format!("commit {}", &r[..7]);
                }
            }
        } else if let Some(t) = tag {
            // 2. Explicit Tag
            let refname = format!("refs/tags/{}", t);
            if let Ok(r_ref) = repo.find_reference(&refname) {
                if let Ok(obj) = r_ref.peel_to_commit() {
                    obj_to_checkout = Some(obj.into_object());
                    checkout_msg = format!("tag {}", t);
                }
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
                if let Ok(r_ref) = repo.find_branch(&remote_ref, git2::BranchType::Remote) {
                    if let Ok(obj) = r_ref.get().peel_to_commit() {
                        obj_to_checkout = Some(obj.into_object());
                        checkout_msg = format!("branch {}", b);
                    }
                }
            }
        } else if let Some(rev) = locked_commit {
            // 4. Lockfile Commit (Zero Config Reproducibility)
            if let Ok(oid) = git2::Oid::from_str(&rev) {
                if let Ok(obj) = repo.find_object(oid, None) {
                    obj_to_checkout = Some(obj);
                    checkout_msg = format!("locked {}", &rev[..7]);
                }
            }
        }

        if let Some(obj) = obj_to_checkout {
            let _ = repo.set_head_detached(obj.id());
            let _ = repo.checkout_tree(&obj, None);
            println!("   {} Locked to {}", "📌".blue(), checkout_msg);
        }

        // Update Lockfile with current HEAD
        if let Ok(head) = repo.head() {
            if let Ok(target) = head.peel_to_commit() {
                let current_hash = target.id().to_string();
                lockfile.insert(name.clone(), url.clone(), current_hash);
            }
        }

        // C. Build Custom Script (If any)
        if let Some(cmd_str) = build_script {
            let out_filename = output_file.as_deref().unwrap_or("");
            let should_build = if !out_filename.is_empty() {
                !lib_path.join(out_filename).exists()
            } else {
                true
            };

            if should_build {
                println!("   {} Building {}...", "🔨".yellow(), name);
                let status = if cfg!(target_os = "windows") {
                    Command::new("cmd")
                        .args(&["/C", &cmd_str])
                        .current_dir(&lib_path)
                        .status()
                } else {
                    Command::new("sh")
                        .args(&["-c", &cmd_str])
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

        // D. Register Includes Flags
        include_flags.push(format!("-I{}", lib_path.display()));
        include_flags.push(format!("-I{}/include", lib_path.display()));
        include_flags.push(format!("-I{}/src", lib_path.display()));

        // E. Smart Linking Logic (Zero Config Header-Only Support)
        if let Some(out_file) = output_file {
            let full_lib_path = lib_path.join(out_file);
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

    lockfile.save()?;
    Ok((include_flags, link_flags))
}
