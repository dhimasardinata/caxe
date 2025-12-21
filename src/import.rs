use crate::config::{BuildConfig, CxConfig, PackageConfig};
use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan_project(path: &Path) -> Result<Option<CxConfig>> {
    println!("{} Scanning directory...", "⚡".yellow());

    // 1. Detect Source Files
    let mut sources = Vec::new();
    let mut has_cpp = false;
    let mut has_c = false;
    let mut include_dirs = Vec::new();

    // Check for "include" directory
    if path.join("include").exists() {
        include_dirs.push("include".to_string());
    }

    // Walk directory (ignoring build, .git, etc.)
    for entry in WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.to_string_lossy().contains("build") || p.to_string_lossy().contains(".git") {
            continue;
        }

        if let Some(ext) = p.extension() {
            let ext_str = ext.to_string_lossy();
            if ext_str == "cpp" || ext_str == "cc" || ext_str == "cxx" {
                has_cpp = true;
                sources.push(p.to_path_buf());
            } else if ext_str == "c" {
                has_c = true;
                sources.push(p.to_path_buf());
            }
        }
    }

    if !has_cpp && !has_c {
        println!("{} No C/C++ source files found.", "x".red());
        return Ok(None);
    }

    // 2. Guess Project Name
    let name = if path.join("CMakeLists.txt").exists() {
        // Try to parse project(NAME)
        if let Ok(content) = fs::read_to_string(path.join("CMakeLists.txt")) {
            let re = regex::Regex::new(r"project\s*\(\s*(\w+)").unwrap();
            if let Some(caps) = re.captures(&content) {
                caps.get(1).map(|m| m.as_str().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
    .unwrap_or_else(|| {
        path.file_name()
            .unwrap_or(std::ffi::OsStr::new("my-project"))
            .to_string_lossy()
            .to_string()
    });

    // 3. Construct Config
    let compiler = {
        #[cfg(windows)]
        {
            "msvc".to_string()
        }
        #[cfg(not(windows))]
        {
            if has_cpp {
                "g++".to_string()
            } else {
                "gcc".to_string()
            }
        }
    };

    let mut cflags = Vec::new();
    // Modern defaults
    cflags.push("-Wall".to_string());
    cflags.push("-Wextra".to_string());

    // Add detect includes
    for inc in include_dirs {
        cflags.push(format!("-I{}", inc));
    }

    let config = CxConfig {
        package: PackageConfig {
            name,
            version: "0.1.0".to_string(),
            edition: if has_cpp {
                "c++20".to_string()
            } else {
                "c17".to_string()
            },
        },
        build: Some(BuildConfig {
            compiler: Some(compiler),
            bin: Some("app".to_string()),
            cflags: Some(cflags),
            libs: None, // Hard to guess libs from source
            pch: None,
        }),
        dependencies: None, // Hard to guess deps
        scripts: None,
        test: None,
    };

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_scan_cpp_project() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("caxe_test_import");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(temp_dir.join("src"))?;

        let main_cpp = temp_dir.join("src/main.cpp");
        let mut f = fs::File::create(&main_cpp)?;
        writeln!(f, "int main() {{ return 0; }}")?;

        let config = scan_project(&temp_dir)?.expect("Should detect project");

        assert_eq!(config.package.name, "caxe_test_import");
        assert_eq!(config.package.edition, "c++20");
        let build_cfg = config.build.as_ref().unwrap();
        let compiler = build_cfg.compiler.as_ref().unwrap();
        assert!(
            compiler.contains("msvc")
                || compiler.contains("clang")
                || compiler.contains("gcc")
                || compiler.contains("g++")
        );

        // cleanup
        fs::remove_dir_all(&temp_dir)?;
        Ok(())
    }
}
