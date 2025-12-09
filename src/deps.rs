use crate::config::Dependency;
use anyhow::{Context, Result};
use colored::*;
use git2::Repository;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn fetch_dependencies(
    deps: &HashMap<String, Dependency>,
) -> Result<(Vec<String>, Vec<String>)> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");
    fs::create_dir_all(&cache_dir)?;

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

        // B. Pinning / Checkout Logic (v0.1.5 Feature)
        let mut obj_to_checkout = None;
        let mut checkout_msg = String::new();

        if let Some(r) = rev {
            // 1. Checkout specific commit hash
            if let Ok(oid) = git2::Oid::from_str(&r) {
                if let Ok(obj) = repo.find_object(oid, None) {
                    obj_to_checkout = Some(obj);
                    checkout_msg = format!("commit {}", &r[..7]);
                }
            }
        } else if let Some(t) = tag {
            // 2. Checkout specific Tag
            let refname = format!("refs/tags/{}", t);
            if let Ok(r_ref) = repo.find_reference(&refname) {
                if let Ok(obj) = r_ref.peel_to_commit() {
                    obj_to_checkout = Some(obj.into_object());
                    checkout_msg = format!("tag {}", t);
                }
            }
        } else if let Some(b) = branch {
            // 3. Checkout specific Branch
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
        }

        if let Some(obj) = obj_to_checkout {
            let _ = repo.set_head_detached(obj.id());
            let _ = repo.checkout_tree(&obj, None);
            println!("   {} Locked to {}", "📌".blue(), checkout_msg);
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

    Ok((include_flags, link_flags))
}

pub fn add_dependency(
    lib_input: &str,
    tag: Option<String>,
    branch: Option<String>,
    rev: Option<String>,
) -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    // 1. Parse Input (Short format vs URL)
    // 1. Parse Input (Alias -> Short format -> URL)
    let (name, url) = if let Some(resolved_url) = crate::registry::resolve_alias(lib_input) {
        // Case A: Alias found (e.g. "raylib")
        (lib_input.to_string(), resolved_url)
    } else if lib_input.contains("http") || lib_input.contains("git@") {
        // Case B: Direct URL
        let name = lib_input
            .split('/')
            .last()
            .unwrap_or("unknown")
            .replace(".git", "");
        (name, lib_input.to_string())
    } else {
        // Case C: user/repo
        let parts: Vec<&str> = lib_input.split('/').collect();
        if parts.len() != 2 {
            println!("{} Invalid format. Use 'alias', 'user/repo', or full URL.", "x".red());
            return Ok(());
        }
        let name = parts[1].to_string();
        let url = format!("https://github.com/{}.git", lib_input);
        (name, url)
    };

    println!("{} Adding dependency: {}...", "📦".blue(), name.bold());

    // 2. Load Config
    let config_str = fs::read_to_string("cx.toml")?;
    let mut config: crate::config::CxConfig = toml::from_str(&config_str)?;

    if config.dependencies.is_none() {
        config.dependencies = Some(HashMap::new());
    }

    // 3. Construct Dependency Entry
    let dep_entry = if tag.is_none() && branch.is_none() && rev.is_none() {
        Dependency::Simple(url.clone())
    } else {
        Dependency::Complex {
            git: Some(url.clone()),
            pkg: None,
            branch,
            tag,
            rev,
            build: None,
            output: None,
        }
    };

    // 4. Insert & Save
    if let Some(deps) = &mut config.dependencies {
        if deps.contains_key(&name) {
            println!("{} Dependency '{}' updated.", "!", name);
        }
        deps.insert(name.clone(), dep_entry);
    }

    let new_toml = toml::to_string_pretty(&config)?;
    fs::write("cx.toml", new_toml)?;

    println!("{} Added {} to cx.toml", "✓".green(), name);

    // 5. Fetch immediately
    if let Some(deps) = &config.dependencies {
        let _ = fetch_dependencies(deps)?;
    }

    Ok(())
}

pub fn remove_dependency(name: &str) -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    let config_str = fs::read_to_string("cx.toml")?;
    let mut config: crate::config::CxConfig = toml::from_str(&config_str)?;

    let mut found = false;
    if let Some(deps) = &mut config.dependencies {
        if deps.remove(name).is_some() {
            found = true;
        }
    }

    if found {
        let new_toml = toml::to_string_pretty(&config)?;
        fs::write("cx.toml", new_toml)?;
        println!("{} Removed dependency: {}", "🗑️".red(), name.bold());
    } else {
        println!(
            "{} Dependency '{}' not found in cx.toml",
            "!".yellow(),
            name
        );
    }

    Ok(())
}

pub fn update_dependencies() -> Result<()> {
    if !Path::new("cx.toml").exists() {
        println!("{} Error: cx.toml not found.", "x".red());
        return Ok(());
    }

    println!("{} Checking for updates...", "🔄".blue());

    let config_str = fs::read_to_string("cx.toml")?;
    let config: crate::config::CxConfig = toml::from_str(&config_str)?;

    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");

    if let Some(deps) = config.dependencies {
        for (name, dep_data) in deps {
            let is_git = match dep_data {
                crate::config::Dependency::Simple(_) => true,
                crate::config::Dependency::Complex { git: Some(_), .. } => true,
                _ => false,
            };

            if is_git {
                let lib_path = cache_dir.join(&name);
                if lib_path.exists() {
                    print!("   Updating {} ... ", name);

                    if let Ok(repo) = git2::Repository::open(&lib_path) {
                        // Fetch origin
                        let mut remote = repo.find_remote("origin")?;
                        remote.fetch(&["HEAD"], None, None)?;

                        let status = if cfg!(target_os = "windows") {
                            Command::new("cmd")
                                .args(&["/C", "git pull origin HEAD"])
                                .current_dir(&lib_path)
                                .output()
                        } else {
                            Command::new("sh")
                                .args(&["-c", "git pull origin HEAD"])
                                .current_dir(&lib_path)
                                .output()
                        };

                        if let Ok(out) = status {
                            if out.status.success() {
                                println!("{}", "✓".green());
                            } else {
                                println!("{} (git pull failed)", "x".red());
                            }
                        } else {
                            println!("{}", "Error executing git".red());
                        }
                    } else {
                        println!("{}", "Not a valid git repo".yellow());
                    }
                }
            }
        }
    }

    println!("{} Dependencies updated.", "✓".green());
    Ok(())
}
