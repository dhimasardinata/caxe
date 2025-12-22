//! Windows-specific toolchain discovery using vswhere and vcvars

use super::types::{CompilerType, Toolchain, ToolchainError, VSInstallation};
use colored::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Known paths where vswhere.exe might be located
const VSWHERE_PATHS: &[&str] = &[
    r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    r"C:\Program Files\Microsoft Visual Studio\Installer\vswhere.exe",
];

/// Known paths where standalone LLVM might be installed
const LLVM_PATHS: &[&str] = &[
    r"C:\Program Files\LLVM\bin",
    r"C:\Program Files (x86)\LLVM\bin",
];

/// Find vswhere.exe on the system
pub fn find_vswhere() -> Option<PathBuf> {
    for path in VSWHERE_PATHS {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Query vswhere for Visual Studio installations
pub fn detect_vs_installations() -> Result<Vec<VSInstallation>, ToolchainError> {
    let vswhere = find_vswhere().ok_or_else(|| {
        ToolchainError::VsWhereError(
            "vswhere.exe not found. Please install Visual Studio or Build Tools.".to_string(),
        )
    })?;

    let output = Command::new(&vswhere)
        .args([
            "-all",
            "-format",
            "json",
            "-utf8",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        ])
        .output()?;

    if !output.status.success() {
        return Err(ToolchainError::VsWhereError(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    parse_vswhere_output(&json_str)
}

/// Parse vswhere JSON output
fn parse_vswhere_output(json_str: &str) -> Result<Vec<VSInstallation>, ToolchainError> {
    let installations: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|e| {
        ToolchainError::VsWhereError(format!("Failed to parse vswhere output: {}", e))
    })?;

    let mut result = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    for inst in installations {
        if let (Some(path), Some(name), Some(version), Some(product)) = (
            inst.get("installationPath").and_then(|v| v.as_str()),
            inst.get("displayName").and_then(|v| v.as_str()),
            inst.get("installationVersion").and_then(|v| v.as_str()),
            inst.get("productId").and_then(|v| v.as_str()),
        ) {
            let path_buf = PathBuf::from(path);
            if seen_paths.contains(&path_buf) {
                continue;
            }
            seen_paths.insert(path_buf.clone());

            result.push(VSInstallation {
                install_path: path_buf,
                display_name: name.to_string(),
                version: version.to_string(),
                product_id: product.to_string(),
            });
        }
    }

    Ok(result)
}

/// Find the MSVC toolset path within a VS installation
pub fn find_msvc_toolset(vs_path: &Path) -> Option<(PathBuf, String)> {
    let vc_tools_path = vs_path.join("VC").join("Tools").join("MSVC");
    if !vc_tools_path.exists() {
        return None;
    }

    // Find the latest version directory
    let mut versions: Vec<_> = std::fs::read_dir(&vc_tools_path)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    versions.sort();
    let latest = versions.pop()?;

    let toolset_path = vc_tools_path.join(&latest);
    Some((toolset_path, latest))
}

/// Find cl.exe within MSVC toolset
pub fn find_cl_exe(toolset_path: &Path) -> Option<PathBuf> {
    // Try x64 first, then x86
    for host in ["Hostx64", "Hostx86"] {
        for target in ["x64", "x86"] {
            let cl_path = toolset_path
                .join("bin")
                .join(host)
                .join(target)
                .join("cl.exe");
            if cl_path.exists() {
                return Some(cl_path);
            }
        }
    }
    None
}

/// Find clang-cl bundled with Visual Studio
pub fn find_bundled_clang_cl(vs_path: &Path) -> Option<PathBuf> {
    // VS 2019+ bundles clang in VC\Tools\Llvm
    let paths = [
        vs_path
            .join("VC")
            .join("Tools")
            .join("Llvm")
            .join("x64")
            .join("bin")
            .join("clang-cl.exe"),
        vs_path
            .join("VC")
            .join("Tools")
            .join("Llvm")
            .join("bin")
            .join("clang-cl.exe"),
    ];

    paths.into_iter().find(|p| p.exists())
}

/// Find clang++ bundled with Visual Studio (regular clang, not clang-cl)
pub fn find_bundled_clang(vs_path: &Path) -> Option<PathBuf> {
    // VS bundles clang++ in VC\Tools\Llvm
    let paths = [
        vs_path
            .join("VC")
            .join("Tools")
            .join("Llvm")
            .join("x64")
            .join("bin")
            .join("clang++.exe"),
        vs_path
            .join("VC")
            .join("Tools")
            .join("Llvm")
            .join("bin")
            .join("clang++.exe"),
    ];

    paths.into_iter().find(|p| p.exists())
}

/// Find standalone LLVM installation
pub fn find_standalone_llvm() -> Option<PathBuf> {
    for path in LLVM_PATHS {
        let clang_cl = PathBuf::from(path).join("clang-cl.exe");
        if clang_cl.exists() {
            return Some(clang_cl);
        }
    }

    // Also check registry (HKLM\SOFTWARE\LLVM\LLVM)
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::*;

        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\LLVM\LLVM")
            && let Ok(path) = hklm.get_value::<String, _>("")
        {
            let clang_cl = PathBuf::from(&path).join("bin").join("clang-cl.exe");
            if clang_cl.exists() {
                return Some(clang_cl);
            }
        }
    }

    None
}

/// Find standalone LLVM installation clang++ (not clang-cl)
pub fn find_standalone_clang() -> Option<PathBuf> {
    for path in LLVM_PATHS {
        let clang_pp = PathBuf::from(path).join("clang++.exe");
        if clang_pp.exists() {
            return Some(clang_pp);
        }
    }

    // Also check registry (HKLM\SOFTWARE\LLVM\LLVM)
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::*;

        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\LLVM\LLVM")
            && let Ok(path) = hklm.get_value::<String, _>("")
        {
            let clang_pp = PathBuf::from(&path).join("bin").join("clang++.exe");
            if clang_pp.exists() {
                return Some(clang_pp);
            }
        }
    }

    None
}

/// Load environment variables from vcvars64.bat
pub fn load_vcvars_env(vs_path: &Path) -> Result<HashMap<String, String>, ToolchainError> {
    let vcvars_path = vs_path
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat");

    if !vcvars_path.exists() {
        return Err(ToolchainError::VcVarsError(format!(
            "vcvars64.bat not found at {}",
            vcvars_path.display()
        )));
    }

    // Run vcvars64.bat and capture environment
    // Use `call` to run the batch file - this handles spaces in paths correctly
    let vcvars_str = vcvars_path.to_string_lossy();

    // Build the command string - note: the path *does* need quotes for spaces
    // but we need to ensure they're not double-escaped
    let cmd_str = format!("call \"{}\" && set", vcvars_str);

    // Use raw_arg to avoid double-escaping on Windows
    #[cfg(windows)]
    let output = {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .raw_arg(format!("/C {}", cmd_str))
            .output()?
    };

    #[cfg(not(windows))]
    let output = Command::new("cmd").args(["/C", &cmd_str]).output()?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Check if we got any environment variables in output
    // Don't rely on exit status as cmd.exe can be unreliable
    if output_str.is_empty() || !output_str.contains("Path=") {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ToolchainError::VcVarsError(format!(
            "Failed to load vcvars environment. Exit code: {:?}, stderr: {}",
            output.status.code(),
            stderr
        )));
    }

    let mut env_vars = HashMap::new();

    for line in output_str.lines() {
        if let Some((key, value)) = line.split_once('=') {
            // Only capture relevant variables
            let key_upper = key.to_uppercase();
            if key_upper == "PATH"
                || key_upper == "INCLUDE"
                || key_upper == "LIB"
                || key_upper == "LIBPATH"
                || key_upper.starts_with("VS")
                || key_upper.starts_with("VSCMD")
                || key_upper.starts_with("WINDOWS")
                || key_upper == "UCRTVERSION"
                || key_upper == "VCTOOLSVERSION"
            {
                env_vars.insert(key.to_string(), value.to_string());
            }
        }
    }

    Ok(env_vars)
}

/// Get compiler version string
fn get_compiler_version(compiler_path: &Path, is_msvc: bool) -> String {
    let output = if is_msvc {
        Command::new(compiler_path).output()
    } else {
        Command::new(compiler_path).arg("--version").output()
    };

    match output {
        Ok(out) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );

            for line in combined.lines() {
                let text = line.trim();
                if text.is_empty() {
                    continue;
                }

                if is_msvc {
                    // Ignore copyright/usage/banner noise
                    if text.starts_with("Copyright") || text.starts_with("usage:") {
                        continue;
                    }
                    // "Microsoft (R) C/C++ Optimizing Compiler Version 19.xx..."
                    if text.contains("Microsoft (R)") && text.contains("Version") {
                        return text.to_string();
                    }
                    // Fallback: if we haven't found exact match but it looks like a version line
                    if text.contains("Version") {
                        return text.to_string();
                    }
                } else {
                    // GCC/Clang usually puts version on first line
                    return text.to_string();
                }
            }

            // If strictly MSVC and failed to find "Version", try first non-usage line or just return "unknown"
            if is_msvc {
                // Try to pick the first line that isn't usage or copyright as a fallback
                for line in combined.lines() {
                    let text = line.trim();
                    if !text.is_empty()
                        && !text.starts_with("usage:")
                        && !text.starts_with("Copyright")
                    {
                        return text.to_string();
                    }
                }
            }

            "unknown".to_string()
        }
        Err(_) => "unknown".to_string(),
    }
}

/// Main entry point: detect the best available toolchain
pub fn detect_toolchain(preferred: Option<CompilerType>) -> Result<Toolchain, ToolchainError> {
    // 1. Detect VS Installations
    let vs_installations = detect_vs_installations().unwrap_or_default();

    // 2. Detect Installed/PATH MinGW
    let mut mingw_path = None;
    if let Some(home) = dirs::home_dir() {
        let p = home
            .join(".cx")
            .join("tools")
            .join("mingw64")
            .join("bin")
            .join("g++.exe");
        if p.exists() {
            mingw_path = Some(p);
        }
    }
    if mingw_path.is_none() {
        // Try PATH
        if let Ok(output) = std::process::Command::new("where").arg("g++").output()
            && output.status.success()
            && let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                mingw_path = Some(PathBuf::from(line.trim()));
            }
    }

    // 3. Selection Logic
    match preferred {
        Some(CompilerType::GCC) => {
            if let Some(gxx) = mingw_path {
                let version = get_compiler_version(&gxx, false);
                return Ok(Toolchain {
                    compiler_type: CompilerType::GCC,
                    cc_path: gxx.with_file_name("gcc.exe"), // assume gcc next to g++
                    cxx_path: gxx,
                    linker_path: PathBuf::new(), // GCC handles linking
                    version,
                    msvc_toolset_version: None,
                    windows_sdk_version: None,
                    vs_install_path: None,
                    env_vars: HashMap::new(),
                });
            } else {
                return Err(ToolchainError::NotFound(
                    "GCC/MinGW not found. Run 'cx toolchain install mingw'.".to_string(),
                ));
            }
        }
        Some(CompilerType::MSVC) => {
            if vs_installations.is_empty() {
                return Err(ToolchainError::NotFound(
                    "Visual Studio not found.".to_string(),
                ));
            }
        }
        _ => {
            // No preference, or Clang/ClangCL.
            // If VS missing, can we fallback to GCC?
            if vs_installations.is_empty() {
                if let Some(gxx) = mingw_path {
                    println!(
                        "{} Visual Studio not found, falling back to MinGW.",
                        "!".yellow()
                    );
                    let version = get_compiler_version(&gxx, false);
                    return Ok(Toolchain {
                        compiler_type: CompilerType::GCC,
                        cc_path: gxx.with_file_name("gcc.exe"),
                        cxx_path: gxx,
                        linker_path: PathBuf::new(),
                        version,
                        msvc_toolset_version: None,
                        windows_sdk_version: None,
                        vs_install_path: None,
                        env_vars: HashMap::new(),
                    });
                }
                return Err(ToolchainError::NotFound(
                    "No suitable compiler found (VS or MinGW).".to_string(),
                ));
            }
        }
    }

    // VS is present if we got here (unless we returned GCC)
    let vs = &vs_installations[0];

    // Load vcvars environment
    let env_vars = load_vcvars_env(&vs.install_path)?;

    // Find MSVC toolset
    let (toolset_path, toolset_version) = find_msvc_toolset(&vs.install_path).ok_or_else(|| {
        ToolchainError::NotFound("MSVC toolset not found in VS installation".to_string())
    })?;

    // Get Windows SDK version from env
    let windows_sdk_version = env_vars.get("WINDOWSSDKVERSION").cloned();

    // Decide which compiler to use based on preference
    let (compiler_type, cxx_path, cc_path) = match preferred {
        Some(CompilerType::ClangCL) => {
            // Try bundled clang-cl in ALL VS installations, then standalone
            let mut clang_cl_path = None;

            // Search all VS installations for clang-cl
            for vs_inst in &vs_installations {
                if let Some(path) = find_bundled_clang_cl(&vs_inst.install_path) {
                    clang_cl_path = Some(path);
                    break;
                }
            }

            // Try standalone LLVM if not found in VS
            if clang_cl_path.is_none() {
                clang_cl_path = find_standalone_llvm();
            }

            if let Some(clang_cl) = clang_cl_path {
                (CompilerType::ClangCL, clang_cl.clone(), clang_cl)
            } else {
                // Fallback to MSVC
                let cl = find_cl_exe(&toolset_path)
                    .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
                (CompilerType::MSVC, cl.clone(), cl)
            }
        }
        Some(CompilerType::MSVC) | None => {
            // Use MSVC (default)
            let cl = find_cl_exe(&toolset_path)
                .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
            (CompilerType::MSVC, cl.clone(), cl)
        }
        Some(CompilerType::Clang) => {
            // Try bundled clang++ in ALL VS installations, then standalone LLVM
            let mut clang_path = None;

            // First try standalone LLVM (user explicitly installed it, may prefer it)
            if let Some(path) = find_standalone_clang() {
                clang_path = Some(path);
            }

            // Then search VS installations for bundled clang++
            if clang_path.is_none() {
                for vs_inst in &vs_installations {
                    if let Some(path) = find_bundled_clang(&vs_inst.install_path) {
                        clang_path = Some(path);
                        break;
                    }
                }
            }

            if let Some(clang) = clang_path {
                let cc_path = clang.with_file_name("clang.exe");
                (CompilerType::Clang, clang, cc_path)
            } else {
                // Fallback to MSVC
                let cl = find_cl_exe(&toolset_path)
                    .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
                (CompilerType::MSVC, cl.clone(), cl)
            }
        }
        Some(CompilerType::GCC) => unreachable!(), // Handled above
    };

    // Get linker path (link.exe for MSVC-compatible)
    let linker_path = toolset_path
        .join("bin")
        .join("Hostx64")
        .join("x64")
        .join("link.exe");

    let version = get_compiler_version(&cxx_path, compiler_type == CompilerType::MSVC);

    Ok(Toolchain {
        compiler_type,
        cc_path,
        cxx_path,
        linker_path,
        version,
        msvc_toolset_version: Some(toolset_version),
        windows_sdk_version,
        vs_install_path: Some(vs.install_path.clone()),
        env_vars,
    })
}

/// Represents an available toolchain option for interactive selection
#[derive(Debug, Clone)]
pub struct AvailableToolchain {
    pub display_name: String,
    pub compiler_type: CompilerType,
    pub path: PathBuf,
    pub version: String,
    pub source: String, // e.g., "VS 2022", "VS 2019", "Standalone LLVM", "MSYS2"
}

impl std::fmt::Display for AvailableToolchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) - {}",
            self.display_name, self.version, self.source
        )
    }
}

/// Discover all available toolchains on the system
pub fn discover_all_toolchains() -> Vec<AvailableToolchain> {
    let mut toolchains = Vec::new();

    // 1. Find all VS installations and their compilers
    if let Ok(vs_installations) = detect_vs_installations() {
        for vs in &vs_installations {
            // MSVC (cl.exe)
            if let Some((toolset_path, _version)) = find_msvc_toolset(&vs.install_path)
                && let Some(cl) = find_cl_exe(&toolset_path)
            {
                let version = get_compiler_version(&cl, true);
                toolchains.push(AvailableToolchain {
                    display_name: "MSVC (cl.exe)".to_string(),
                    compiler_type: CompilerType::MSVC,
                    path: cl,
                    version,
                    source: vs.display_name.clone(),
                });
            }

            // Bundled Clang-CL
            if let Some(clang_cl) = find_bundled_clang_cl(&vs.install_path) {
                let version = get_compiler_version(&clang_cl, false);
                toolchains.push(AvailableToolchain {
                    display_name: "Clang-CL (clang-cl.exe)".to_string(),
                    compiler_type: CompilerType::ClangCL,
                    path: clang_cl,
                    version,
                    source: format!("{} bundled", vs.display_name),
                });
            }

            // Bundled Clang++
            if let Some(clang) = find_bundled_clang(&vs.install_path) {
                let version = get_compiler_version(&clang, false);
                toolchains.push(AvailableToolchain {
                    display_name: "Clang (clang++.exe)".to_string(),
                    compiler_type: CompilerType::Clang,
                    path: clang,
                    version,
                    source: format!("{} bundled", vs.display_name),
                });
            }
        }
    }

    // 2. Standalone LLVM
    if let Some(clang_cl) = find_standalone_llvm() {
        let version = get_compiler_version(&clang_cl, false);
        toolchains.push(AvailableToolchain {
            display_name: "Clang-CL (clang-cl.exe)".to_string(),
            compiler_type: CompilerType::ClangCL,
            path: clang_cl,
            version,
            source: "Standalone LLVM".to_string(),
        });
    }

    if let Some(clang) = find_standalone_clang() {
        let version = get_compiler_version(&clang, false);
        toolchains.push(AvailableToolchain {
            display_name: "Clang (clang++.exe)".to_string(),
            compiler_type: CompilerType::Clang,
            path: clang,
            version,
            source: "Standalone LLVM".to_string(),
        });
    }

    // 3. Installed MinGW (via caxe)
    if let Some(home) = dirs::home_dir() {
        let mingw_bin = home
            .join(".cx")
            .join("tools")
            .join("mingw64")
            .join("bin")
            .join("g++.exe");
        if mingw_bin.exists() {
            let version = get_compiler_version(&mingw_bin, false);
            toolchains.push(AvailableToolchain {
                display_name: "GCC (g++.exe)".to_string(),
                compiler_type: CompilerType::GCC,
                path: mingw_bin,
                version,
                source: "Max/MinGW (WinLibs)".to_string(),
            });
        }
    }

    // 4. GCC from PATH (MSYS2/MinGW)
    if let Ok(output) = std::process::Command::new("where").arg("g++").output()
        && output.status.success()
    {
        let paths = String::from_utf8_lossy(&output.stdout);
        for line in paths.lines() {
            let path = PathBuf::from(line.trim());
            // Avoid duplicates if PATH includes the installed one
            if toolchains.iter().any(|t| t.path == path) {
                continue;
            }

            if path.exists() {
                let version = get_compiler_version(&path, false);
                let source = if line.contains("msys64") {
                    "MSYS2/MinGW"
                } else if line.contains("mingw") {
                    "MinGW"
                } else {
                    "PATH"
                };
                toolchains.push(AvailableToolchain {
                    display_name: "GCC (g++.exe)".to_string(),
                    compiler_type: CompilerType::GCC,
                    path,
                    version,
                    source: source.to_string(),
                });
                break; // Only take first g++ found
            }
        }
    }

    toolchains
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_vswhere() {
        // This test will only pass on systems with VS installed
        if let Some(path) = find_vswhere() {
            assert!(path.exists());
            assert!(path.to_string_lossy().contains("vswhere"));
        }
    }
}
