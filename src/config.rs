//! Configuration file parsing for `cx.toml`.
//!
//! This module defines the structure of the caxe configuration file and provides
//! parsing/serialization utilities.
//!
//! # Example
//!
//! ```toml
//! [package]
//! name = "myapp"
//! version = "1.0.0"
//! edition = "c++20"
//!
//! [build]
//! compiler = "clang++"
//! flags = ["-O2", "-Wall"]
//!
//! [dependencies]
//! fmt = "https://github.com/fmtlib/fmt"
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration structure parsed from `cx.toml`.
///
/// This is the main configuration type that contains all project settings
/// including package metadata, build options, dependencies, and profiles.
#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct CxConfig {
    /// Package metadata (name, version, edition).
    pub package: PackageConfig,
    /// Optional dependencies (git URLs or complex configs).
    pub dependencies: Option<HashMap<String, Dependency>>,
    /// Optional build configuration (compiler, flags, libs).
    pub build: Option<BuildConfig>,
    /// Optional pre/post build scripts.
    pub scripts: Option<ScriptsConfig>,
    /// Optional test configuration.
    pub test: Option<TestConfig>,
    /// Optional workspace configuration for multi-project setups.
    pub workspace: Option<WorkspaceConfig>,
    /// Optional Arduino/IoT configuration.
    pub arduino: Option<ArduinoConfig>,
    /// Named profiles for cross-compilation: [profile:name]
    #[serde(skip)]
    pub profiles: HashMap<String, Profile>,
}

/// Build profile for cross-compilation
/// Used with [profile:name] sections in cx.toml
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Profile {
    /// Base profile to inherit from (e.g., "release", "debug")
    pub base: Option<String>,
    /// Target triple (e.g., "xtensa-esp32-elf", "aarch64-linux-gnu")
    pub target: Option<String>,
    /// Compiler override
    pub compiler: Option<String>,
    /// Compiler flags (preferred over cflags)
    pub flags: Option<Vec<String>>,
    /// Libraries to link
    pub libs: Option<Vec<String>>,
    /// Output binary name override
    pub bin: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct ArduinoConfig {
    /// Arduino board FQBN (e.g., "arduino:avr:uno", "esp32:esp32:esp32")
    pub board: Option<String>,
    /// Port for upload (e.g., "COM3", "/dev/ttyUSB0")
    pub port: Option<String>,
    /// Additional arduino-cli flags
    pub flags: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct WorkspaceConfig {
    pub members: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct TestConfig {
    pub framework: Option<String>,
    pub source_dir: Option<String>,
    pub single_binary: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Dependency {
    // Case: "https://github.com/..."
    Simple(String),

    // Case: { git = "...", tag = "v1.0" }
    Complex {
        git: Option<String>,
        pkg: Option<String>,
        // Pinning Features
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        // Build Features
        build: Option<String>,
        output: Option<String>,
    },
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct PackageConfig {
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
    #[serde(default = "default_edition")]
    pub edition: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct BuildConfig {
    pub compiler: Option<String>,
    pub bin: Option<String>,
    /// Compiler flags (new, preferred)
    pub flags: Option<Vec<String>>,
    /// Deprecated: use `flags` instead
    pub cflags: Option<Vec<String>>,
    pub libs: Option<Vec<String>>,
    /// Linker flags (e.g., /SUBSYSTEM:WINDOWS)
    pub ldflags: Option<Vec<String>>,
    pub sources: Option<Vec<String>>,
    pub pch: Option<String>,
    /// Windows subsystem (console or windows)
    pub subsystem: Option<String>,
    /// Framework to use (e.g., "daxe", "arduino")
    /// Automatically fetches and includes framework headers
    pub framework: Option<String>,
    /// Include paths for compilation
    pub include: Option<Vec<String>>,
    /// Build type (e.g., "header-only", "library", "executable")
    #[serde(rename = "type")]
    pub build_type: Option<String>,
    /// Terminal encoding: "utf-8" (default) or "system"
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

impl BuildConfig {
    /// Get effective flags, preferring `flags` over deprecated `cflags`
    pub fn get_flags(&self) -> Option<&Vec<String>> {
        self.flags.as_ref().or(self.cflags.as_ref())
    }

    /// Check if using deprecated cflags field
    pub fn uses_deprecated_cflags(&self) -> bool {
        self.cflags.is_some() && self.flags.is_none()
    }
}

fn default_edition() -> String {
    "c++23".to_string()
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct ScriptsConfig {
    pub pre_build: Option<String>,
    pub post_build: Option<String>,
}

pub fn create_ephemeral_config(
    name: &str,
    bin_name: &str,
    compiler: &str,
    has_cpp: bool,
) -> CxConfig {
    CxConfig {
        package: PackageConfig {
            name: name.to_string(),
            version: "0.0.0".to_string(),
            edition: if has_cpp {
                "c++23".to_string()
            } else {
                "c23".to_string()
            },
        },
        build: Some(BuildConfig {
            compiler: Some(compiler.to_string()),
            bin: Some(bin_name.to_string()),
            flags: None,
            cflags: None,
            libs: None,
            ldflags: None,
            sources: Some(vec![name.to_string()]),
            pch: None,
            subsystem: None,
            framework: None,
            include: None,
            build_type: None,
            encoding: default_encoding(),
        }),
        dependencies: None,
        scripts: None,
        test: None,
        workspace: None,
        arduino: None,
        profiles: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_config_get_flags_prefers_flags() {
        let config = BuildConfig {
            flags: Some(vec!["-O2".to_string()]),
            cflags: Some(vec!["-O0".to_string()]),
            ..Default::default()
        };
        let flags = config.get_flags().unwrap();
        assert_eq!(flags[0], "-O2");
    }

    #[test]
    fn test_build_config_get_flags_falls_back_to_cflags() {
        let config = BuildConfig {
            flags: None,
            cflags: Some(vec!["-Wall".to_string()]),
            ..Default::default()
        };
        let flags = config.get_flags().unwrap();
        assert_eq!(flags[0], "-Wall");
    }

    #[test]
    fn test_build_config_uses_deprecated_cflags() {
        let deprecated = BuildConfig {
            cflags: Some(vec![]),
            flags: None,
            ..Default::default()
        };
        let modern = BuildConfig {
            flags: Some(vec![]),
            cflags: None,
            ..Default::default()
        };
        assert!(deprecated.uses_deprecated_cflags());
        assert!(!modern.uses_deprecated_cflags());
    }

    #[test]
    fn test_create_ephemeral_config_cpp() {
        let config = create_ephemeral_config("main.cpp", "app", "g++", true);
        assert_eq!(config.package.name, "main.cpp");
        assert_eq!(config.package.edition, "c++23");
        assert!(config.build.is_some());
        let build = config.build.unwrap();
        assert_eq!(build.compiler, Some("g++".to_string()));
        assert_eq!(build.bin, Some("app".to_string()));
    }

    #[test]
    fn test_create_ephemeral_config_c() {
        let config = create_ephemeral_config("main.c", "prog", "gcc", false);
        assert_eq!(config.package.edition, "c23");
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
[package]
name = "test"
version = "1.0.0"
"#;
        let config: CxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.package.name, "test");
        assert_eq!(config.package.version, "1.0.0");
        assert_eq!(config.package.edition, "c++23"); // default
    }

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
[package]
name = "myapp"
version = "2.0.0"
edition = "c++20"

[build]
compiler = "clang++"
bin = "myapp"
flags = ["-O3", "-Wall"]
libs = ["pthread"]
"#;
        let config: CxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.package.edition, "c++20");
        let build = config.build.unwrap();
        assert_eq!(build.compiler, Some("clang++".to_string()));
        assert_eq!(
            build.flags,
            Some(vec!["-O3".to_string(), "-Wall".to_string()])
        );
    }

    #[test]
    fn test_dependency_simple() {
        let toml_str = r#"
[package]
name = "test"
version = "1.0.0"

[dependencies]
glfw = "https://github.com/glfw/glfw"
"#;
        let config: CxConfig = toml::from_str(toml_str).unwrap();
        let deps = config.dependencies.unwrap();
        match &deps["glfw"] {
            Dependency::Simple(url) => assert!(url.contains("github.com")),
            _ => panic!("Expected Simple dependency"),
        }
    }

    #[test]
    fn test_dependency_complex() {
        let toml_str = r#"
[package]
name = "test"
version = "1.0.0"

[dependencies]
sdl2 = { git = "https://github.com/libsdl-org/SDL", tag = "release-2.30.0" }
"#;
        let config: CxConfig = toml::from_str(toml_str).unwrap();
        let deps = config.dependencies.unwrap();
        match &deps["sdl2"] {
            Dependency::Complex { git, tag, .. } => {
                assert!(git.as_ref().unwrap().contains("SDL"));
                assert_eq!(tag.as_ref().unwrap(), "release-2.30.0");
            }
            _ => panic!("Expected Complex dependency"),
        }
    }
}
