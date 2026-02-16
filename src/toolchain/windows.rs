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

fn first_existing_path<I>(paths: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    paths.into_iter().find(|p| p.exists())
}

fn find_in_llvm_paths(binary: &str) -> Option<PathBuf> {
    first_existing_path(
        LLVM_PATHS
            .iter()
            .map(|base| PathBuf::from(base).join(binary)),
    )
}

#[cfg(windows)]
fn find_from_llvm_registry(binary: &str) -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::*;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\LLVM\LLVM")
        .ok()?;
    let path = hklm.get_value::<String, _>("").ok()?;
    let candidate = PathBuf::from(path).join("bin").join(binary);
    candidate.exists().then_some(candidate)
}

#[cfg(not(windows))]
fn find_from_llvm_registry(_binary: &str) -> Option<PathBuf> {
    None
}

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
    find_in_llvm_paths("clang-cl.exe").or_else(|| find_from_llvm_registry("clang-cl.exe"))
}

/// Find standalone LLVM installation clang++ (not clang-cl)
pub fn find_standalone_clang() -> Option<PathBuf> {
    find_in_llvm_paths("clang++.exe").or_else(|| find_from_llvm_registry("clang++.exe"))
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

/// Detect a toolchain from a specific VS source (matched by display name)
/// This is used when the user has selected a specific VS installation
pub fn detect_toolchain_from_source(
    compiler_type: CompilerType,
    source: &str,
) -> Result<Toolchain, ToolchainError> {
    // Get all VS installations
    let vs_installations = detect_vs_installations().unwrap_or_default();

    // Find the matching installation by source name
    let vs = vs_installations
        .iter()
        .find(|vs| source.contains(&vs.display_name) || vs.display_name.contains(source))
        .ok_or_else(|| {
            ToolchainError::NotFound(format!(
                "Visual Studio installation '{}' not found. Run 'cx toolchain select' to choose again.",
                source
            ))
        })?;

    // Load vcvars environment from the specific VS installation
    let env_vars = load_vcvars_env(&vs.install_path)?;

    // Find MSVC toolset in this specific installation
    let (toolset_path, toolset_version) = find_msvc_toolset(&vs.install_path).ok_or_else(|| {
        ToolchainError::NotFound("MSVC toolset not found in selected VS installation".to_string())
    })?;

    // Get Windows SDK version from env
    let windows_sdk_version = env_vars.get("WINDOWSSDKVERSION").cloned();

    // Decide which compiler to use based on type
    let (final_type, cxx_path, cc_path) = match compiler_type {
        CompilerType::ClangCL => {
            if let Some(clang_cl) = find_bundled_clang_cl(&vs.install_path) {
                (CompilerType::ClangCL, clang_cl.clone(), clang_cl)
            } else if let Some(clang_cl) = find_standalone_llvm() {
                (CompilerType::ClangCL, clang_cl.clone(), clang_cl)
            } else {
                let cl = find_cl_exe(&toolset_path)
                    .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
                (CompilerType::MSVC, cl.clone(), cl)
            }
        }
        CompilerType::Clang => {
            if let Some(clang) = find_bundled_clang(&vs.install_path) {
                let cc = clang.with_file_name("clang.exe");
                (CompilerType::Clang, clang, cc)
            } else if let Some(clang) = find_standalone_clang() {
                let cc = clang.with_file_name("clang.exe");
                (CompilerType::Clang, clang, cc)
            } else {
                let cl = find_cl_exe(&toolset_path)
                    .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
                (CompilerType::MSVC, cl.clone(), cl)
            }
        }
        CompilerType::MSVC | CompilerType::GCC => {
            let cl = find_cl_exe(&toolset_path)
                .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
            (CompilerType::MSVC, cl.clone(), cl)
        }
    };

    // Get linker path
    let linker_path = toolset_path
        .join("bin")
        .join("Hostx64")
        .join("x64")
        .join("link.exe");

    let version = get_compiler_version(&cxx_path, final_type == CompilerType::MSVC);

    Ok(Toolchain {
        compiler_type: final_type,
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

/// Main entry point: detect the best available toolchain
pub fn detect_toolchain(preferred: Option<CompilerType>) -> Result<Toolchain, ToolchainError> {
    let vs_installations = detect_vs_installations().unwrap_or_default();
    let mingw_path = detect_mingw_path();
    if let Some(toolchain) = maybe_resolve_without_vs(&preferred, &vs_installations, mingw_path)? {
        return Ok(toolchain);
    }

    let vs = &vs_installations[0];
    let env_vars = load_vcvars_env(&vs.install_path)?;
    let (toolset_path, toolset_version) = find_msvc_toolset(&vs.install_path).ok_or_else(|| {
        ToolchainError::NotFound("MSVC toolset not found in VS installation".to_string())
    })?;
    let windows_sdk_version = env_vars.get("WINDOWSSDKVERSION").cloned();
    let (compiler_type, cxx_path, cc_path) =
        select_vs_compiler(&preferred, &vs_installations, &toolset_path)?;

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

fn detect_mingw_path() -> Option<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        let installed = home
            .join(".cx")
            .join("tools")
            .join("mingw64")
            .join("bin")
            .join("g++.exe");
        if installed.exists() {
            return Some(installed);
        }
    }

    if let Ok(output) = Command::new("where").arg("g++").output()
        && output.status.success()
        && let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next()
    {
        return Some(PathBuf::from(line.trim()));
    }

    None
}

fn gcc_toolchain(gxx: PathBuf) -> Toolchain {
    let version = get_compiler_version(&gxx, false);
    Toolchain {
        compiler_type: CompilerType::GCC,
        cc_path: gxx.with_file_name("gcc.exe"),
        cxx_path: gxx,
        linker_path: PathBuf::new(),
        version,
        msvc_toolset_version: None,
        windows_sdk_version: None,
        vs_install_path: None,
        env_vars: HashMap::new(),
    }
}

fn maybe_resolve_without_vs(
    preferred: &Option<CompilerType>,
    vs_installations: &[VSInstallation],
    mingw_path: Option<PathBuf>,
) -> Result<Option<Toolchain>, ToolchainError> {
    match preferred {
        Some(CompilerType::GCC) => mingw_path.map(gcc_toolchain).map(Some).ok_or_else(|| {
            ToolchainError::NotFound(
                "GCC/MinGW not found. Run 'cx toolchain install mingw'.".to_string(),
            )
        }),
        Some(CompilerType::MSVC) => {
            if vs_installations.is_empty() {
                Err(ToolchainError::NotFound(
                    "Visual Studio not found.".to_string(),
                ))
            } else {
                Ok(None)
            }
        }
        _ => {
            if !vs_installations.is_empty() {
                return Ok(None);
            }
            if let Some(gxx) = mingw_path {
                println!(
                    "{} Visual Studio not found, falling back to MinGW.",
                    "!".yellow()
                );
                Ok(Some(gcc_toolchain(gxx)))
            } else {
                Err(ToolchainError::NotFound(
                    "No suitable compiler found (VS or MinGW).".to_string(),
                ))
            }
        }
    }
}

fn find_first_bundled_clang_cl(vs_installations: &[VSInstallation]) -> Option<PathBuf> {
    vs_installations
        .iter()
        .find_map(|vs| find_bundled_clang_cl(&vs.install_path))
}

fn find_first_bundled_clang(vs_installations: &[VSInstallation]) -> Option<PathBuf> {
    vs_installations
        .iter()
        .find_map(|vs| find_bundled_clang(&vs.install_path))
}

fn msvc_compiler(toolset_path: &Path) -> Result<(CompilerType, PathBuf, PathBuf), ToolchainError> {
    let cl = find_cl_exe(toolset_path)
        .ok_or_else(|| ToolchainError::NotFound("cl.exe not found".to_string()))?;
    Ok((CompilerType::MSVC, cl.clone(), cl))
}

fn select_vs_compiler(
    preferred: &Option<CompilerType>,
    vs_installations: &[VSInstallation],
    toolset_path: &Path,
) -> Result<(CompilerType, PathBuf, PathBuf), ToolchainError> {
    match preferred {
        Some(CompilerType::ClangCL) => {
            let clang_cl = find_first_bundled_clang_cl(vs_installations)
                .or_else(find_standalone_llvm)
                .map(|path| (CompilerType::ClangCL, path.clone(), path));
            clang_cl
                .map(Ok)
                .unwrap_or_else(|| msvc_compiler(toolset_path))
        }
        Some(CompilerType::Clang) => {
            let clang = find_standalone_clang()
                .or_else(|| find_first_bundled_clang(vs_installations))
                .map(|path| {
                    (
                        CompilerType::Clang,
                        path.clone(),
                        path.with_file_name("clang.exe"),
                    )
                });
            clang.map(Ok).unwrap_or_else(|| msvc_compiler(toolset_path))
        }
        Some(CompilerType::GCC) => unreachable!(),
        Some(CompilerType::MSVC) | None => msvc_compiler(toolset_path),
    }
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
    add_vs_toolchains(&mut toolchains);
    add_standalone_llvm_toolchains(&mut toolchains);
    add_installed_mingw_toolchain(&mut toolchains);
    add_path_mingw_toolchain(&mut toolchains);
    toolchains
}

fn push_toolchain(
    toolchains: &mut Vec<AvailableToolchain>,
    display_name: &str,
    compiler_type: CompilerType,
    path: PathBuf,
    source: String,
    is_msvc: bool,
) {
    let version = get_compiler_version(&path, is_msvc);
    toolchains.push(AvailableToolchain {
        display_name: display_name.to_string(),
        compiler_type,
        path,
        version,
        source,
    });
}

fn add_vs_toolchains(toolchains: &mut Vec<AvailableToolchain>) {
    if let Ok(vs_installations) = detect_vs_installations() {
        for vs in &vs_installations {
            if let Some((toolset_path, _version)) = find_msvc_toolset(&vs.install_path)
                && let Some(cl) = find_cl_exe(&toolset_path)
            {
                push_toolchain(
                    toolchains,
                    "MSVC (cl.exe)",
                    CompilerType::MSVC,
                    cl,
                    vs.display_name.clone(),
                    true,
                );
            }

            if let Some(clang_cl) = find_bundled_clang_cl(&vs.install_path) {
                push_toolchain(
                    toolchains,
                    "Clang-CL (clang-cl.exe)",
                    CompilerType::ClangCL,
                    clang_cl,
                    format!("{} bundled", vs.display_name),
                    false,
                );
            }

            if let Some(clang) = find_bundled_clang(&vs.install_path) {
                push_toolchain(
                    toolchains,
                    "Clang (clang++.exe)",
                    CompilerType::Clang,
                    clang,
                    format!("{} bundled", vs.display_name),
                    false,
                );
            }
        }
    }
}

fn add_standalone_llvm_toolchains(toolchains: &mut Vec<AvailableToolchain>) {
    if let Some(clang_cl) = find_standalone_llvm() {
        push_toolchain(
            toolchains,
            "Clang-CL (clang-cl.exe)",
            CompilerType::ClangCL,
            clang_cl,
            "Standalone LLVM".to_string(),
            false,
        );
    }

    if let Some(clang) = find_standalone_clang() {
        push_toolchain(
            toolchains,
            "Clang (clang++.exe)",
            CompilerType::Clang,
            clang,
            "Standalone LLVM".to_string(),
            false,
        );
    }
}

fn add_installed_mingw_toolchain(toolchains: &mut Vec<AvailableToolchain>) {
    if let Some(home) = dirs::home_dir() {
        let mingw_bin = home
            .join(".cx")
            .join("tools")
            .join("mingw64")
            .join("bin")
            .join("g++.exe");
        if mingw_bin.exists() {
            push_toolchain(
                toolchains,
                "GCC (g++.exe)",
                CompilerType::GCC,
                mingw_bin,
                "Max/MinGW (WinLibs)".to_string(),
                false,
            );
        }
    }
}

fn path_source_label(path_line: &str) -> &'static str {
    if path_line.contains("msys64") {
        "MSYS2/MinGW"
    } else if path_line.contains("mingw") {
        "MinGW"
    } else {
        "PATH"
    }
}

fn add_path_mingw_toolchain(toolchains: &mut Vec<AvailableToolchain>) {
    if let Ok(output) = Command::new("where").arg("g++").output()
        && output.status.success()
    {
        let paths = String::from_utf8_lossy(&output.stdout);
        for line in paths.lines() {
            let path = PathBuf::from(line.trim());
            if toolchains.iter().any(|t| t.path == path) {
                continue;
            }

            if path.exists() {
                push_toolchain(
                    toolchains,
                    "GCC (g++.exe)",
                    CompilerType::GCC,
                    path,
                    path_source_label(line).to_string(),
                    false,
                );
                break;
            }
        }
    }
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
