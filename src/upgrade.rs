use anyhow::{Context, Result};
use colored::*;
use semver::Version;
use serde::Deserialize;
use std::env;
use std::fs;

const REPO_OWNER: &str = "dhimasardinata";
const REPO_NAME: &str = "caxe";

#[derive(Deserialize, Debug)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize, Debug)]
struct Asset {
    name: String,
    browser_download_url: String,
}

use std::time::Duration;

pub fn check_and_upgrade() -> Result<()> {
    println!("{} Checking for updates...", "🔍".blue());

    let current_ver = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );

    let resp = ureq::get(&url)
        .set("User-Agent", "caxe-updater")
        .timeout(Duration::from_secs(10))
        .call()
        .context("Failed to check for updates")?;

    let release: Release = resp.into_json()?;

    // Clean tag name (remove 'v' prefix if present)
    let tag_clean = release.tag_name.trim_start_matches('v');
    let remote_ver = Version::parse(tag_clean).context("Failed to parse remote version")?;

    if remote_ver <= current_ver {
        println!("{} caxe is up to date (v{})", "✓".green(), current_ver);
        return Ok(());
    }

    println!(
        "{} New version available: v{} -> v{}",
        "🚀".green(),
        current_ver,
        remote_ver
    );
    println!("Downloading...");

    // Find Asset
    let target = get_target_name();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(target))
        .or_else(|| {
            // Fallback heuristics
            if cfg!(windows) && release.assets.iter().any(|a| a.name.ends_with(".exe")) {
                release.assets.iter().find(|a| a.name.ends_with(".exe"))
            } else {
                None
            }
        })
        .context("No compatible binary found for this OS")?;

    // Download
    let agent = ureq::get(&asset.browser_download_url)
        .set("User-Agent", "caxe-updater")
        .call()
        .context("Failed to download update")?;

    let total_size = agent
        .header("content-length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let pb = indicatif::ProgressBar::new(total_size);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("Downloading...");

    let mut reader = agent.into_reader();
    let current_exe = env::current_exe()?;
    let tmp_exe = current_exe.with_extension("tmp");
    let mut tmp_file = fs::File::create(&tmp_exe)?;

    // Copy with progress
    let mut buffer = [0; 8192];
    use std::io::Read;
    use std::io::Write;
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        tmp_file.write_all(&buffer[..n])?;
        pb.inc(n as u64);
    }
    pb.finish_with_message("Download complete");

    // Replace
    println!("Installing...");

    if cfg!(target_os = "windows") {
        let old_exe = current_exe.with_extension("line_old");
        // Rename current to .old (allowed on Windows)
        if old_exe.exists() {
            let _ = fs::remove_file(&old_exe);
        }
        let _ = fs::rename(&current_exe, &old_exe);
        fs::rename(&tmp_exe, &current_exe)?;
    } else {
        // Unix: can override running file usually, or rename.
        // Rename is safer.
        fs::rename(&tmp_exe, &current_exe)?;
        // Make executable (chmod +x)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&current_exe)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&current_exe, perms)?;
        }
    }

    println!("{} Successfully upgraded to v{}!", "✓".green(), remote_ver);
    Ok(())
}

fn get_target_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos-arm64"
        } else {
            "macos-intel"
        }
    } else {
        "linux"
    }
}
