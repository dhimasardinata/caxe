use crate::build;
use anyhow::Result;
use colored::*;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::FileOptions;

pub fn package_project(output_name: Option<String>, release: bool) -> Result<()> {
    // 1. Build the project first
    println!("{} Building project for packaging...", "📦".blue());
    let config = build::load_config()?;

    // Force release build for packaging usually, unless specified otherwise?
    // CLI generic args might control this too, but package command usually implies release.
    // However, we'll respect the `release` flag passed in.

    // We need to run the build command. Since `build_project` takes config and options,
    // we can construct options here.
    let build_opts = build::BuildOptions {
        release,
        verbose: false,
        dry_run: false,
        enable_profile: false,
        wasm: false,
        lto: true, // optimize for size/speed for package
        sanitize: None,
    };

    if let Err(e) = build::build_project(&config, &build_opts) {
        return Err(anyhow::anyhow!("Build failed: {}", e));
    }

    // 2. Determine Output Paths
    let project_name = config.package.name.clone();
    let version = config.package.version.clone();

    let build_dir = if release {
        Path::new("build").join("release")
    } else {
        Path::new("build").join("debug")
    };

    let binary_name = if cfg!(windows) {
        format!("{}.exe", project_name)
    } else {
        project_name.clone()
    };

    let binary_path = build_dir.join(&binary_name);

    if !binary_path.exists() {
        return Err(anyhow::anyhow!(
            "Binary not found at: {}",
            binary_path.display()
        ));
    }

    // Determine config output name
    let zip_filename = output_name.unwrap_or_else(|| format!("{}-v{}.zip", project_name, version));

    // Output inside build directory to keep root clean
    let zip_path = Path::new("build").join(&zip_filename);

    println!("{} Creating archive: {}", "💾".blue(), zip_path.display());

    let file = File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // 3. Add Binary
    println!("   {} Adding executable: {}", "+".green(), binary_name);
    zip.start_file(&binary_name, options)?;
    let mut f = File::open(&binary_path)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    zip.write_all(&buffer)?;

    // 4. Add Assets (if exist)
    if Path::new("assets").exists() {
        println!("   {} Adding assets...", "+".green());
        let walk = WalkDir::new("assets");
        for entry in walk {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Determine name in zip
                // e.g. assets/subdir -> assets/subdir/
                // zip crate handles dirs by adding a file ending in / usually, or implied by files.
                // We can explicitly add directories if we want empty ones,
                // but usually adding files is enough.
                // zip.add_directory(name, options)?;
                continue;
            }

            let name = path
                .strip_prefix(Path::new("."))
                .unwrap_or(path)
                .to_string_lossy();

            // Avoid adding non-files or weird system files if necessary

            #[cfg(windows)]
            let name = name.replace("\\", "/"); // Zip standard uses forward slashes

            zip.start_file(name, options)?;
            let mut f = File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        }
    }

    // 5. Add Dynamic Libraries (DLLs) - Best Effort
    // On Windows, users often need DLLs next to exe.
    // If we have a vendor directory or know about deps, we could try to copy them.
    // For now, let's look for .dll files in the build directory that might have been copied there during build?
    // Or just skip for MVP.
    // Let's scan the `build_dir` for any OTHER .dll files and include them.
    if cfg!(windows)
        && let Ok(entries) = std::fs::read_dir(&build_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                        && ext == "dll" {
                            let name = path.file_name().unwrap().to_string_lossy();
                            println!("   {} Adding library: {}", "+".green(), name);
                            zip.start_file(name, options)?;
                            let mut f = File::open(&path)?;
                            let mut buffer = Vec::new();
                            f.read_to_end(&mut buffer)?;
                            zip.write_all(&buffer)?;
                        }
            }
        }

    zip.finish()?;

    println!("{} Package ready: {}", "✓".green(), zip_path.display());
    Ok(())
}
