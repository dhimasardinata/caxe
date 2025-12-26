use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LockFile {
    #[serde(rename = "package")]
    pub packages: BTreeMap<String, PackageLock>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageLock {
    pub git: String,
    pub rev: String,
}

impl LockFile {
    pub fn load() -> Result<Self> {
        if Path::new("cx.lock").exists() {
            let content = fs::read_to_string("cx.lock")?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write("cx.lock", content)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&PackageLock> {
        self.packages.get(name)
    }

    pub fn insert(&mut self, name: String, git: String, rev: String) {
        self.packages.insert(name, PackageLock { git, rev });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockfile_insert_and_get() {
        let mut lock = LockFile::default();
        lock.insert(
            "fmt".to_string(),
            "https://github.com/fmtlib/fmt".to_string(),
            "abc123".to_string(),
        );
        let entry = lock.get("fmt").unwrap();
        assert_eq!(entry.git, "https://github.com/fmtlib/fmt");
        assert_eq!(entry.rev, "abc123");
    }

    #[test]
    fn test_lockfile_get_missing() {
        let lock = LockFile::default();
        assert!(lock.get("nonexistent").is_none());
    }

    #[test]
    fn test_lockfile_serialization() {
        let mut lock = LockFile::default();
        lock.insert(
            "json".to_string(),
            "https://github.com/nlohmann/json".to_string(),
            "v3.11.2".to_string(),
        );
        let toml_str = toml::to_string_pretty(&lock).unwrap();
        assert!(toml_str.contains("json"));
        assert!(toml_str.contains("v3.11.2"));
    }

    #[test]
    fn test_lockfile_parse() {
        let toml_str = r#"
[package]
fmt = { git = "https://github.com/fmtlib/fmt", rev = "abc123" }
"#;
        let lock: LockFile = toml::from_str(toml_str).unwrap();
        let entry = lock.get("fmt").unwrap();
        assert_eq!(entry.rev, "abc123");
    }
}
