//! Portable toolchain installation.
//!
//! This module provides the `cx toolchain install` command for downloading
//! and installing portable C++ compilers.
//!
//! ## Supported Toolchains
//!
//! - GCC (MinGW-w64) - Portable GCC for Windows
//! - Clang/LLVM - Portable LLVM toolchain
//! - Arduino CLI - For embedded development

use anyhow::{Context, Result};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{Confirm, Select};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Toolchain installation options
#[derive(Debug, Clone, PartialEq)]
pub enum ToolchainChoice {
    GCC,
    Clang,
    ClangCL,
    MSVC,
    Skip,
}

/// Build system options (info only)
#[derive(Debug, Clone, PartialEq)]
pub enum BuildSystemChoice {
    CMake,
    Ninja,
    Conan,
    Vcpkg,
    Make,
    Skip,
}

/// Main entry point for toolchain installation
pub fn install_toolchain(name: Option<String>) -> Result<()> {
    match name {
        Some(n) => install_by_name(&n),
        None => install_interactive(),
    }
}

/// Interactive installation wizard
fn install_interactive() -> Result<()> {
    println!();
    println!("{} {}", "🔧".cyan(), "Toolchain Installation Wizard".bold());
    println!("{}", "─".repeat(40).dimmed());
    println!();

    // Question 1: Compiler Toolchain
    let toolchain_options = get_toolchain_options();
    let toolchain_choice = Select::new("Select a toolchain to install:", toolchain_options)
        .with_help_message("Choose a compiler toolchain for your system")
        .prompt()?;

    match parse_toolchain_choice(toolchain_choice) {
        ToolchainChoice::GCC => install_gcc_adaptive()?,
        ToolchainChoice::Clang => install_clang_adaptive()?,
        ToolchainChoice::ClangCL => {
            #[cfg(windows)]
            install_clang_cl_info()?;
            #[cfg(not(windows))]
            println!("{} Clang-CL is only available on Windows.", "!".yellow());
        }
        ToolchainChoice::MSVC => {
            #[cfg(windows)]
            install_msvc_info()?;
            #[cfg(not(windows))]
            println!("{} MSVC is only available on Windows.", "!".yellow());
        }
        ToolchainChoice::Skip => {
            println!("{} Skipping toolchain installation.", "→".dimmed());
        }
    }

    println!();

    // Question 2: Build System / Dependency Manager
    let build_system_options = vec![
        "CMake - Cross-platform build system",
        "Ninja - Fast build system",
        "Conan - C/C++ package manager",
        "vcpkg - Microsoft's package manager",
        "Make - Traditional Unix build tool",
        "[Skip]",
    ];

    let build_choice = Select::new(
        "Install build tools? (Note: Not yet fully integrated with caxe):",
        build_system_options,
    )
    .with_help_message("These tools can be used alongside caxe")
    .prompt()?;

    match build_choice {
        s if s.starts_with("CMake") => show_install_info("CMake", "https://cmake.org/download/"),
        s if s.starts_with("Ninja") => show_install_info("Ninja", "https://ninja-build.org/"),
        s if s.starts_with("Conan") => {
            println!("{} Install Conan via pip:", "💡".cyan());
            println!("   {}", "pip install conan".yellow());
        }
        s if s.starts_with("vcpkg") => show_install_info("vcpkg", "https://vcpkg.io/"),
        s if s.starts_with("Make") => {
            #[cfg(windows)]
            println!(
                "{} Make is included with MinGW. Run {} first.",
                "💡".cyan(),
                "cx toolchain install gcc".yellow()
            );
            #[cfg(not(windows))]
            println!(
                "{} Make is usually pre-installed. Try: {}",
                "💡".cyan(),
                "sudo apt install build-essential".yellow()
            );
        }
        _ => println!("{} Skipping build tools.", "→".dimmed()),
    }

    println!();

    // Question 3: clang-format
    let install_format = Confirm::new("Install clang-format for code formatting?")
        .with_default(true)
        .with_help_message("Used by 'cx fmt' command")
        .prompt()?;

    if install_format {
        install_clang_format()?;
    } else {
        println!("{} Skipping clang-format.", "→".dimmed());
    }

    println!();

    // Question 4: Additional Development Tools
    let dev_tools_options = vec![
        "Git - Version control system",
        "Emscripten - WebAssembly/asm.js compiler",
        "Doxygen - Documentation generator",
        "cppcheck - Static code analyzer",
        "ESP-IDF - ESP32 development framework",
        "Arduino - Arduino development framework",
        "CMake - Cross-platform build system",
        "Ninja - Fast build system",
        "[Done - Skip additional tools]",
    ];

    loop {
        let tool_choice = Select::new(
            "Install additional development tools?",
            dev_tools_options.clone(),
        )
        .with_help_message("Select a tool to install, or Done to finish")
        .prompt()?;

        match tool_choice {
            s if s.starts_with("Git") => install_git_info()?,
            s if s.starts_with("Emscripten") => install_emscripten_info()?,
            s if s.starts_with("Doxygen") => install_doxygen_info()?,
            s if s.starts_with("cppcheck") => install_cppcheck_info()?,
            s if s.starts_with("ESP-IDF") => install_espidf_info()?,
            s if s.starts_with("Arduino") => install_arduino_info()?,
            s if s.starts_with("CMake") => install_cmake_info()?,
            s if s.starts_with("Ninja") => install_ninja_info()?,
            _ => {
                println!("{} Finished selecting tools.", "✓".green());
                break;
            }
        }
        println!();
    }

    println!();
    println!("{} Installation wizard complete!", "✓".green());
    Ok(())
}

/// Get OS-appropriate toolchain options
fn get_toolchain_options() -> Vec<&'static str> {
    #[cfg(windows)]
    {
        vec![
            "GCC (MinGW-w64) - GNU Compiler Collection",
            "Clang (LLVM) - Modern compiler with great diagnostics",
            "Clang-CL - Clang with MSVC compatibility",
            "MSVC - Microsoft Visual C++",
            "[Skip]",
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            "GCC - GNU Compiler Collection (via Homebrew)",
            "Clang (LLVM) - Already included with Xcode",
            "[Skip]",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            "GCC - GNU Compiler Collection",
            "Clang (LLVM) - Modern compiler with great diagnostics",
            "[Skip]",
        ]
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        vec!["GCC - GNU Compiler Collection", "Clang (LLVM)", "[Skip]"]
    }
}

fn parse_toolchain_choice(choice: &str) -> ToolchainChoice {
    if choice.starts_with("GCC") {
        ToolchainChoice::GCC
    } else if choice.starts_with("Clang-CL") {
        ToolchainChoice::ClangCL
    } else if choice.starts_with("Clang") {
        ToolchainChoice::Clang
    } else if choice.starts_with("MSVC") {
        ToolchainChoice::MSVC
    } else {
        ToolchainChoice::Skip
    }
}

/// Install by specific name (non-interactive)
fn install_by_name(name: &str) -> Result<()> {
    match name.to_lowercase().as_str() {
        "mingw" | "gcc" => install_gcc_adaptive(),
        "llvm" | "clang" => install_clang_adaptive(),
        "clang-cl" => {
            #[cfg(windows)]
            {
                install_clang_cl_info()
            }
            #[cfg(not(windows))]
            {
                println!("{} Clang-CL is only available on Windows.", "!".yellow());
                Ok(())
            }
        }
        "msvc" => {
            #[cfg(windows)]
            {
                install_msvc_info()
            }
            #[cfg(not(windows))]
            {
                println!("{} MSVC is only available on Windows.", "!".yellow());
                Ok(())
            }
        }
        "clang-format" => install_clang_format(),
        _ => {
            println!(
                "{} Unknown toolchain '{}'. Supported: gcc, clang, clang-cl, msvc, clang-format",
                "x".red(),
                name
            );
            Ok(())
        }
    }
}

/// Install GCC based on OS
fn install_gcc_adaptive() -> Result<()> {
    #[cfg(windows)]
    {
        install_mingw()
    }
    #[cfg(target_os = "macos")]
    {
        println!("{} To install GCC on macOS:", "💡".cyan());
        println!("   {}", "brew install gcc".yellow());
        println!();
        println!("   After installation, GCC will be available as gcc-13 (or similar version).");
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        println!("{} To install GCC on Linux:", "💡".cyan());
        println!(
            "   Ubuntu/Debian: {}",
            "sudo apt install build-essential".yellow()
        );
        println!("   Fedora:        {}", "sudo dnf install gcc-c++".yellow());
        println!("   Arch:          {}", "sudo pacman -S gcc".yellow());
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        println!(
            "{} Please install GCC using your system's package manager.",
            "!".yellow()
        );
        Ok(())
    }
}

/// Install Clang based on OS
fn install_clang_adaptive() -> Result<()> {
    #[cfg(windows)]
    {
        println!("{} To install Clang/LLVM on Windows:", "💡".cyan());
        println!();
        println!("   Option 1: Download from LLVM releases:");
        println!(
            "   {}",
            "https://github.com/llvm/llvm-project/releases".blue()
        );
        println!();
        println!("   Option 2: Install via winget:");
        println!("   {}", "winget install LLVM.LLVM".yellow());
        println!();
        println!("   Option 3: Install with Visual Studio (includes Clang):");
        println!("   Select 'C++ Clang tools for Windows' component.");
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        println!(
            "{} Clang is included with Xcode Command Line Tools.",
            "✓".green()
        );
        println!("   Install with: {}", "xcode-select --install".yellow());
        println!();
        println!("   For the latest LLVM version:");
        println!("   {}", "brew install llvm".yellow());
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        println!("{} To install Clang/LLVM on Linux:", "💡".cyan());
        println!("   Ubuntu/Debian: {}", "sudo apt install clang".yellow());
        println!("   Fedora:        {}", "sudo dnf install clang".yellow());
        println!("   Arch:          {}", "sudo pacman -S clang".yellow());
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        println!(
            "{} Please install Clang using your system's package manager.",
            "!".yellow()
        );
        Ok(())
    }
}

#[cfg(windows)]
fn install_clang_cl_info() -> Result<()> {
    println!(
        "{} Clang-CL is best installed via Visual Studio:",
        "💡".cyan()
    );
    println!();
    println!("   1. Open Visual Studio Installer");
    println!("   2. Modify your installation");
    println!("   3. Under 'Individual components', select:");
    println!("      {}", "\"C++ Clang tools for Windows\"".yellow());
    println!();
    println!("   Alternatively, install standalone LLVM and use with MSVC libraries.");
    Ok(())
}

#[cfg(windows)]
fn install_msvc_info() -> Result<()> {
    println!("{} To install MSVC:", "💡".cyan());
    println!();
    println!("   1. Download Visual Studio Build Tools:");
    println!(
        "      {}",
        "https://visualstudio.microsoft.com/visual-cpp-build-tools/".blue()
    );
    println!();
    println!("   2. Run the installer and select:");
    println!("      {}", "\"Desktop development with C++\"".yellow());
    println!();
    println!("   3. caxe will automatically detect the installation.");
    Ok(())
}

/// Install clang-format
fn install_clang_format() -> Result<()> {
    #[cfg(windows)]
    {
        println!("{} To install clang-format on Windows:", "💡".cyan());
        println!();
        println!("   Option 1: Install via winget:");
        println!("   {}", "winget install LLVM.LLVM".yellow());
        println!();
        println!("   Option 2: Download standalone from LLVM releases");
        println!();
        println!("   clang-format will be available after adding LLVM/bin to PATH.");
    }
    #[cfg(target_os = "macos")]
    {
        println!(
            "{} clang-format is included with Xcode CLT or LLVM:",
            "💡".cyan()
        );
        println!("   {}", "xcode-select --install".yellow());
        println!("   or: {}", "brew install clang-format".yellow());
    }
    #[cfg(target_os = "linux")]
    {
        println!("{} To install clang-format on Linux:", "💡".cyan());
        println!(
            "   Ubuntu/Debian: {}",
            "sudo apt install clang-format".yellow()
        );
        println!(
            "   Fedora:        {}",
            "sudo dnf install clang-tools-extra".yellow()
        );
        println!("   Arch:          {}", "sudo pacman -S clang".yellow());
    }
    Ok(())
}

fn show_install_info(name: &str, url: &str) {
    println!("{} Download {} from:", "💡".cyan(), name);
    println!("   {}", url.blue());
}

#[cfg(windows)]
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

#[cfg(windows)]
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
        .template("{spinner:.blue} [{elapsed_precise}] [{bar:40.green/black}] {bytes}/{total_bytes} ({eta})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .tick_chars("◐◓◑◒")
        .progress_chars("━━╸"));

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

#[cfg(windows)]
fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

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

/// Update installed toolchains to newer versions
pub fn update_toolchains() -> Result<()> {
    println!(
        "{} {}",
        "🔄".cyan(),
        "Checking for toolchain updates...".bold()
    );
    println!();

    let mut updated = false;

    #[cfg(windows)]
    {
        let home = dirs::home_dir().context("Could not find home directory")?;
        let tool_dir = home.join(".cx").join("tools");
        let mingw_dir = tool_dir.join("mingw64");

        // Check if MinGW is installed via caxe
        if mingw_dir.exists() {
            println!(
                "{} Found installed MinGW at {}",
                "📦".blue(),
                mingw_dir.display()
            );

            // Get current version
            let gcc_path = mingw_dir.join("bin").join("g++.exe");
            let current_version = if gcc_path.exists() {
                std::process::Command::new(&gcc_path)
                    .arg("--version")
                    .output()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .next()
                            .unwrap_or("unknown")
                            .to_string()
                    })
                    .unwrap_or_else(|_| "unknown".to_string())
            } else {
                "unknown".to_string()
            };
            println!("   Current version: {}", current_version.dimmed());

            // Ask user if they want to update
            let update = inquire::Confirm::new("Update MinGW to latest version?")
                .with_default(true)
                .prompt()?;

            if update {
                // Remove old installation
                println!("{} Removing old installation...", "🗑️".red());
                std::fs::remove_dir_all(&mingw_dir)?;

                // Install new version
                install_mingw()?;
                updated = true;
            }
        } else {
            println!("{} No caxe-installed toolchains found.", "!".yellow());
            println!(
                "   Use {} to install toolchains.",
                "cx toolchain install".cyan()
            );
        }
    }

    #[cfg(not(windows))]
    {
        println!(
            "{} Toolchain updates are currently Windows-only.",
            "!".yellow()
        );
        println!("   Use your system package manager to update toolchains:");
        println!("   - macOS: {}", "brew upgrade gcc llvm".yellow());
        println!(
            "   - Ubuntu: {}",
            "sudo apt update && sudo apt upgrade".yellow()
        );
    }

    if updated {
        println!();
        println!("{} Toolchains updated successfully!", "✓".green());
    } else {
        println!();
        println!("{} No updates performed.", "!".yellow());
    }

    Ok(())
}

// --- Development Tool Installation Info Functions ---

fn install_git_info() -> Result<()> {
    println!("{} {}", "📦".cyan(), "Git - Version Control System".bold());
    println!();

    #[cfg(windows)]
    {
        println!(
            "   Download from: {}",
            "https://git-scm.com/download/win".blue().underline()
        );
        println!();
        println!("   Or install via winget:");
        println!("   {}", "winget install Git.Git".yellow());
        println!();
        println!("   Or via Scoop:");
        println!("   {}", "scoop install git".yellow());
    }

    #[cfg(target_os = "macos")]
    {
        println!("   Install via Homebrew:");
        println!("   {}", "brew install git".yellow());
        println!();
        println!("   Or install Xcode Command Line Tools:");
        println!("   {}", "xcode-select --install".yellow());
    }

    #[cfg(target_os = "linux")]
    {
        println!("   Ubuntu/Debian:");
        println!("   {}", "sudo apt install git".yellow());
        println!();
        println!("   Fedora:");
        println!("   {}", "sudo dnf install git".yellow());
        println!();
        println!("   Arch:");
        println!("   {}", "sudo pacman -S git".yellow());
    }

    Ok(())
}

fn install_emscripten_info() -> Result<()> {
    println!(
        "{} {}",
        "📦".cyan(),
        "Emscripten - WebAssembly Compiler".bold()
    );
    println!();
    println!("   Official installation (recommended):");
    println!(
        "   {}",
        "git clone https://github.com/emscripten-core/emsdk.git".yellow()
    );
    println!("   {}", "cd emsdk".yellow());
    println!("   {}", "emsdk install latest".yellow());
    println!("   {}", "emsdk activate latest".yellow());
    println!();
    println!("   Then add to PATH and run:");
    println!("   {}", "source ./emsdk_env.sh".yellow());
    println!();
    println!(
        "   Docs: {}",
        "https://emscripten.org/docs/getting_started/"
            .blue()
            .underline()
    );
    println!();
    println!(
        "   {} Use {} to build for WebAssembly",
        "💡".cyan(),
        "cx build --wasm".green()
    );

    Ok(())
}

fn install_doxygen_info() -> Result<()> {
    println!(
        "{} {}",
        "📦".cyan(),
        "Doxygen - Documentation Generator".bold()
    );
    println!();

    #[cfg(windows)]
    {
        println!(
            "   Download from: {}",
            "https://www.doxygen.nl/download.html".blue().underline()
        );
        println!();
        println!("   Or install via winget:");
        println!("   {}", "winget install DimitriVanHeesch.Doxygen".yellow());
        println!();
        println!("   Or via Scoop:");
        println!("   {}", "scoop install doxygen".yellow());
    }

    #[cfg(target_os = "macos")]
    {
        println!("   Install via Homebrew:");
        println!("   {}", "brew install doxygen".yellow());
    }

    #[cfg(target_os = "linux")]
    {
        println!("   Ubuntu/Debian:");
        println!("   {}", "sudo apt install doxygen".yellow());
        println!();
        println!("   Fedora:");
        println!("   {}", "sudo dnf install doxygen".yellow());
    }

    println!();
    println!(
        "   {} Use {} to generate docs",
        "💡".cyan(),
        "cx doc".green()
    );

    Ok(())
}

fn install_cppcheck_info() -> Result<()> {
    println!(
        "{} {}",
        "📦".cyan(),
        "cppcheck - Static Code Analyzer".bold()
    );
    println!();

    #[cfg(windows)]
    {
        println!(
            "   Download from: {}",
            "https://cppcheck.sourceforge.io/".blue().underline()
        );
        println!();
        println!("   Or install via winget:");
        println!("   {}", "winget install Cppcheck.Cppcheck".yellow());
        println!();
        println!("   Or via Scoop:");
        println!("   {}", "scoop install cppcheck".yellow());
    }

    #[cfg(target_os = "macos")]
    {
        println!("   Install via Homebrew:");
        println!("   {}", "brew install cppcheck".yellow());
    }

    #[cfg(target_os = "linux")]
    {
        println!("   Ubuntu/Debian:");
        println!("   {}", "sudo apt install cppcheck".yellow());
        println!();
        println!("   Fedora:");
        println!("   {}", "sudo dnf install cppcheck".yellow());
    }

    println!();
    println!(
        "   {} Use {} for static analysis",
        "💡".cyan(),
        "cx check".green()
    );

    Ok(())
}

fn install_espidf_info() -> Result<()> {
    println!(
        "{} {}",
        "📦".cyan(),
        "ESP-IDF - ESP32 Development Framework".bold()
    );
    println!();
    println!("   Official installation guide:");
    println!(
        "   {}",
        "https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/"
            .blue()
            .underline()
    );
    println!();

    #[cfg(windows)]
    {
        println!("   Windows: Download ESP-IDF Tools Installer:");
        println!(
            "   {}",
            "https://dl.espressif.com/dl/esp-idf/".blue().underline()
        );
    }

    #[cfg(not(windows))]
    {
        println!("   Quick start:");
        println!("   {}", "mkdir -p ~/esp && cd ~/esp".yellow());
        println!(
            "   {}",
            "git clone --recursive https://github.com/espressif/esp-idf.git".yellow()
        );
        println!(
            "   {}",
            "cd esp-idf && ./install.sh && . ./export.sh".yellow()
        );
    }

    println!();
    println!(
        "   {} Configure {} and build with {} for ESP-IDF",
        "💡".cyan(),
        "[profile:esp32]".green(),
        "cx build --profile esp32".green()
    );

    Ok(())
}

fn install_cmake_info() -> Result<()> {
    println!(
        "{} {}",
        "📦".cyan(),
        "CMake - Cross-Platform Build System".bold()
    );
    println!();

    #[cfg(windows)]
    {
        println!(
            "   Download from: {}",
            "https://cmake.org/download/".blue().underline()
        );
        println!();
        println!("   Or install via winget:");
        println!("   {}", "winget install Kitware.CMake".yellow());
        println!();
        println!("   Or via Scoop:");
        println!("   {}", "scoop install cmake".yellow());
    }

    #[cfg(target_os = "macos")]
    {
        println!("   Install via Homebrew:");
        println!("   {}", "brew install cmake".yellow());
    }

    #[cfg(target_os = "linux")]
    {
        println!("   Ubuntu/Debian:");
        println!("   {}", "sudo apt install cmake".yellow());
        println!();
        println!("   Fedora:");
        println!("   {}", "sudo dnf install cmake".yellow());
    }

    println!();
    println!(
        "   {} Use {} to generate CMakeLists.txt",
        "💡".cyan(),
        "cx generate cmake".green()
    );

    Ok(())
}

fn install_ninja_info() -> Result<()> {
    println!("{} {}", "📦".cyan(), "Ninja - Fast Build System".bold());
    println!();

    #[cfg(windows)]
    {
        println!(
            "   Download from: {}",
            "https://ninja-build.org/".blue().underline()
        );
        println!();
        println!("   Or install via winget:");
        println!("   {}", "winget install Ninja-build.Ninja".yellow());
        println!();
        println!("   Or via Scoop:");
        println!("   {}", "scoop install ninja".yellow());
    }

    #[cfg(target_os = "macos")]
    {
        println!("   Install via Homebrew:");
        println!("   {}", "brew install ninja".yellow());
    }

    #[cfg(target_os = "linux")]
    {
        println!("   Ubuntu/Debian:");
        println!("   {}", "sudo apt install ninja-build".yellow());
        println!();
        println!("   Fedora:");
        println!("   {}", "sudo dnf install ninja-build".yellow());
    }

    println!();
    println!(
        "   {} Use {} to generate build.ninja",
        "💡".cyan(),
        "cx generate ninja".green()
    );

    Ok(())
}

fn install_arduino_info() -> Result<()> {
    println!(
        "{} {}",
        "📦".cyan(),
        "Arduino - Development Framework".bold()
    );
    println!();
    println!("   Arduino IDE (recommended for beginners):");
    println!(
        "   {}",
        "https://www.arduino.cc/en/software".blue().underline()
    );
    println!();
    println!("   Arduino CLI (for command-line usage):");

    #[cfg(windows)]
    {
        println!("   Install via winget:");
        println!("   {}", "winget install Arduino.Arduino-CLI".yellow());
        println!();
        println!("   Or via Scoop:");
        println!("   {}", "scoop install arduino-cli".yellow());
    }

    #[cfg(target_os = "macos")]
    {
        println!("   Install via Homebrew:");
        println!("   {}", "brew install arduino-cli".yellow());
    }

    #[cfg(target_os = "linux")]
    {
        println!("   Install script:");
        println!("   {}", "curl -fsSL https://raw.githubusercontent.com/arduino/arduino-cli/master/install.sh | sh".yellow());
    }

    println!();
    println!("   PlatformIO (alternative, integrates with VSCode):");
    println!("   {}", "https://platformio.org/".blue().underline());
    println!();
    println!(
        "   {} Arduino uses its own build system, not compatible with caxe",
        "!".yellow()
    );

    Ok(())
}
