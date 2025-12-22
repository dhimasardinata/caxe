use anyhow::{Context, Result};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub fn install_toolchain(name: String) -> Result<()> {
    match name.to_lowercase().as_str() {
        "mingw" | "gcc" => install_mingw(),
        "llvm" | "clang" => {
            println!(
                "{} LLVM installation not yet supported via caxe. Please install LLVM manually.",
                "!".yellow()
            );
            Ok(())
        }
        _ => {
            println!(
                "{} Unknown toolchain '{}'. Supported: mingw",
                "x".red(),
                name
            );
            Ok(())
        }
    }
}

fn install_mingw() -> Result<()> {
    println!("{} Installing MinGW-w64 (WinLibs)...", "📦".cyan());

    // 1. Determine Paths
    let home = dirs::home_dir().context("Could not find home directory")?;
    let tool_dir = home.join(".cx").join("tools");
    let mingw_dir = tool_dir.join("mingw64");

    if mingw_dir.exists() {
        println!(
            "{} MinGW is already installed at: {}",
            "!".yellow(),
            mingw_dir.display()
        );
        println!("   Delete it if you want to reinstall.");
        return Ok(());
    }

    std::fs::create_dir_all(&tool_dir)?;

    // 2. Download
    let url = "https://github.com/brechtsanders/winlibs_mingw/releases/download/13.2.0-16.0.6-11.0.1-msvcrt-r1/winlibs-x86_64-posix-seh-gcc-13.2.0-llvm-16.0.6-mingw-w64msvcrt-11.0.1-r1.zip";
    let zip_path = tool_dir.join("mingw.zip");

    println!("{} Downloading... (approx 400MB)", "⬇".blue());
    download_file(url, &zip_path)?;

    // 3. Extract
    println!("{} Extracting... (this may take a while)", "📦".blue());
    extract_zip(&zip_path, &tool_dir)?;

    // 4. Cleanup
    std::fs::remove_file(&zip_path)?;

    println!("{} MinGW installed successfully!", "✓".green());
    println!("   Location: {}", mingw_dir.display());

    Ok(())
}

fn download_file(url: &str, path: &Path) -> Result<()> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("Download failed: {}", e))?;

    let total_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    let mut file = File::create(path)?;
    let mut reader = response.into_body().into_reader();
    let mut buffer = [0; 8192];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        pb.inc(n as u64);
    }

    pb.finish_with_message("Download complete");
    Ok(())
}

fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // WinLibs zip structure: /mingw64/...
    // So extracting to .cx/tools should result in .cx/tools/mingw64

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent()
                && !p.exists()
            {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}
