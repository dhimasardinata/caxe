use anyhow::{Context, Result};
use colored::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/dhimasardinata/caxe/main/registry.json";
const CACHE_FILE: &str = "registry.json";
const CACHE_TTL_SECS: u64 = 86400; // 24 hours

#[derive(Deserialize, Debug, Clone)]
pub struct RegistryEntry {
    pub url: String,
    pub description: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Registry(HashMap<String, RegistryEntry>);

impl Registry {
    pub fn get(name: &str) -> Option<String> {
        let registry = Self::load().unwrap_or_else(|_| Self::default());
        registry.0.get(name).map(|entry| entry.url.clone())
    }

    #[allow(dead_code)]
    pub fn get_entry(name: &str) -> Option<RegistryEntry> {
        let registry = Self::load().unwrap_or_else(|_| Self::default());
        registry.0.get(name).cloned()
    }

    fn default() -> Self {
        // Fallback hardcoded registry
        let mut m = HashMap::new();
        m.insert(
            "raylib".to_string(),
            RegistryEntry {
                url: "https://github.com/raysan5/raylib.git".to_string(),
                description: Some(
                    "A simple and easy-to-use library to enjoy videogames programming".to_string(),
                ),
            },
        );
        m.insert(
            "json".to_string(),
            RegistryEntry {
                url: "https://github.com/nlohmann/json.git".to_string(),
                description: Some("JSON for Modern C++".to_string()),
            },
        );
        m.insert(
            "fmt".to_string(),
            RegistryEntry {
                url: "https://github.com/fmtlib/fmt.git".to_string(),
                description: Some("A modern formatting library".to_string()),
            },
        );
        Self(m)
    }

    fn load() -> Result<Self> {
        let cache_path = Self::get_cache_path()?;

        // 1. Check Cache Validity
        if let Ok(metadata) = fs::metadata(&cache_path)
            && let Ok(modified) = metadata.modified()
            && let Ok(age) = SystemTime::now().duration_since(modified)
            && age < Duration::from_secs(CACHE_TTL_SECS)
            && let Ok(content) = fs::read_to_string(&cache_path)
            && let Ok(reg) = serde_json::from_str::<HashMap<String, RegistryEntry>>(&content)
        {
            return Ok(Self(reg));
        }

        // 2. Fetch from Remote
        print!("{} Fetching registry... ", "⚡".yellow());
        match ureq::get(REGISTRY_URL).call() {
            Ok(mut response) => {
                let content = response.body_mut().read_to_string()?;
                println!("{}", "✓".green());

                // Save to cache
                if let Some(parent) = cache_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&cache_path, &content)?;

                let map: HashMap<String, RegistryEntry> =
                    serde_json::from_str(&content).context("Failed to parse registry JSON")?;
                Ok(Self(map))
            }
            Err(_) => {
                println!("{}", "Failed (Using cached/fallback)".red());
                // Try reading cache even if old
                if cache_path.exists() {
                    let content = fs::read_to_string(&cache_path)?;
                    let map: HashMap<String, RegistryEntry> = serde_json::from_str(&content)?;
                    Ok(Self(map))
                } else {
                    Ok(Self::default())
                }
            }
        }
    }

    fn get_cache_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not find home directory")?;
        Ok(home.join(".cx").join(CACHE_FILE))
    }
}

pub fn resolve_alias(name: &str) -> Option<String> {
    Registry::get(name)
}

fn ensure_cx_toml_exists() -> Result<()> {
    use std::path::Path;
    if Path::new("cx.toml").exists() {
        Ok(())
    } else {
        anyhow::bail!("No cx.toml found. Run 'cx init' or 'cx new' first.")
    }
}

fn dependency_exists(content: &str, name: &str) -> bool {
    let name_lower = name.to_lowercase();
    content
        .to_lowercase()
        .contains(&format!("{} =", name_lower))
}

fn with_added_dependency(content: &str, dep_line: &str) -> String {
    if content.contains("[dependencies]") {
        content.replace("[dependencies]", &format!("[dependencies]\n{}", dep_line))
    } else {
        format!("{}\n\n[dependencies]\n{}\n", content.trim(), dep_line)
    }
}

fn remove_dependency_line(content: &str, name: &str) -> (String, bool) {
    let name_lower = name.to_lowercase();
    let mut found = false;
    let new_lines: Vec<&str> = content
        .lines()
        .filter(|line| {
            let line_lower = line.to_lowercase().trim_start().to_string();
            let matches = line_lower.starts_with(&format!("{} =", name_lower))
                || line_lower.starts_with(&format!("\"{}\"", name_lower));
            if matches {
                found = true;
            }
            !matches
        })
        .collect();

    (new_lines.join("\n"), found)
}

pub fn search(query: &str) -> Vec<(String, String)> {
    let registry = Registry::load().unwrap_or_else(|_| Registry::default());
    let query = query.to_lowercase();

    registry
        .0
        .iter()
        .filter(|(k, entry)| {
            k.to_lowercase().contains(&query)
                || entry
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query))
                    .unwrap_or(false)
        })
        .map(|(k, entry)| (k.clone(), entry.url.clone()))
        .collect()
}

/// Add a package to cx.toml
pub fn add_package(name: &str) -> Result<()> {
    ensure_cx_toml_exists()?;

    // Get package URL from registry
    let url = resolve_alias(name).ok_or_else(|| {
        anyhow::anyhow!(
            "Package '{}' not found in registry. Try 'cx search {}'",
            name,
            name
        )
    })?;

    // Read existing cx.toml
    let content = fs::read_to_string("cx.toml")?;

    // Check if package already exists
    if dependency_exists(&content, name) {
        println!("   {} {} is already in dependencies", "⚡".yellow(), name);
        return Ok(());
    }

    // Build the dependency line
    let dep_line = format!("{} = \"{}\"", name, url);

    let new_content = with_added_dependency(&content, &dep_line);

    fs::write("cx.toml", new_content)?;

    println!("   {} Added {} to dependencies", "✓".green(), name.cyan());
    println!("   {} {}", "📦".blue(), url);

    Ok(())
}

/// Remove a package from cx.toml
pub fn remove_package(name: &str) -> Result<()> {
    ensure_cx_toml_exists()?;

    let content = fs::read_to_string("cx.toml")?;
    let (new_content, found) = remove_dependency_line(&content, name);
    if !found {
        println!("   {} {} not found in dependencies", "⚠".yellow(), name);
        return Ok(());
    }

    fs::write("cx.toml", new_content)?;

    println!(
        "   {} Removed {} from dependencies",
        "✓".green(),
        name.cyan()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_contains_common_libs() {
        let registry = Registry::default();
        assert!(registry.0.contains_key("raylib"));
        assert!(registry.0.contains_key("json"));
        assert!(registry.0.contains_key("fmt"));
    }

    #[test]
    fn test_registry_entry_has_url() {
        let registry = Registry::default();
        let entry = registry.0.get("raylib").unwrap();
        assert!(entry.url.contains("github.com"));
        assert!(entry.description.is_some());
    }

    #[test]
    fn test_search_finds_by_name() {
        // This tests the search logic pattern, using default registry
        let registry = Registry::default();
        let query = "ray";
        let results: Vec<_> = registry
            .0
            .iter()
            .filter(|(k, _)| k.to_lowercase().contains(&query.to_lowercase()))
            .collect();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_finds_by_description() {
        let registry = Registry::default();
        let query = "json";
        let results: Vec<_> = registry
            .0
            .iter()
            .filter(|(_, entry)| {
                entry
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query.to_lowercase()))
                    .unwrap_or(false)
            })
            .collect();
        assert!(!results.is_empty());
    }
}
