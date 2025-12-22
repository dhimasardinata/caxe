use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct CxConfig {
    pub package: PackageConfig,
    pub dependencies: Option<HashMap<String, Dependency>>,
    pub build: Option<BuildConfig>,
    pub scripts: Option<ScriptsConfig>,
    pub test: Option<TestConfig>,
    pub workspace: Option<WorkspaceConfig>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct WorkspaceConfig {
    pub members: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
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

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct PackageConfig {
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
    #[serde(default = "default_edition")]
    pub edition: String,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct BuildConfig {
    pub compiler: Option<String>,
    pub bin: Option<String>,
    pub cflags: Option<Vec<String>>,
    pub libs: Option<Vec<String>>,
    pub sources: Option<Vec<String>>,
    pub pch: Option<String>,
}

fn default_edition() -> String {
    "c++20".to_string()
}

#[derive(Deserialize, Serialize, Debug, Default)]
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
                "c++20".to_string()
            } else {
                "c17".to_string()
            },
        },
        build: Some(BuildConfig {
            compiler: Some(compiler.to_string()),
            bin: Some(bin_name.to_string()),
            cflags: None,
            libs: None,
            sources: Some(vec![name.to_string()]),
            pch: None,
        }),
        dependencies: None,
        scripts: None,
        test: None,
        workspace: None,
    }
}
