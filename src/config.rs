use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Default)]
pub struct CxConfig {
    pub package: PackageConfig,
    pub dependencies: Option<HashMap<String, String>>,
    pub build: Option<BuildConfig>,
}

#[derive(Deserialize, Debug, Default)]
pub struct PackageConfig {
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
    #[serde(default = "default_edition")]
    pub edition: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct BuildConfig {
    pub cflags: Option<Vec<String>>,
    pub libs: Option<Vec<String>>,
}

fn default_edition() -> String {
    "c++20".to_string()
}
