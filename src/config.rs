use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct CxConfig {
    pub package: PackageConfig,
    pub dependencies: Option<HashMap<String, Dependency>>,
    pub build: Option<BuildConfig>,
    pub scripts: Option<ScriptsConfig>,
    pub test: Option<TestConfig>,
    pub workspace: Option<WorkspaceConfig>,
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
    pub sources: Option<Vec<String>>,
    pub pch: Option<String>,
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
            sources: Some(vec![name.to_string()]),
            pch: None,
        }),
        dependencies: None,
        scripts: None,
        test: None,
        workspace: None,
        arduino: None,
        profiles: HashMap::new(),
    }
}
