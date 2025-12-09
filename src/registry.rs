use anyhow::Result;
use colored::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, Duration};

const REGISTRY_URL: &str = "https://raw.githubusercontent.com/dhimasardinata/caxe/main/registry.json";
const CACHE_FILE: &str = "registry.json";
const CACHE_TTL_SECS: u64 = 86400; // 24 hours

#[derive(Deserialize, Debug)]
pub struct Registry(HashMap<String, String>);

impl Registry {
    pub fn get(name: &str) -> Option<String> {
        let registry = Self::load().unwrap_or_else(|_| Self::default());
        registry.0.get(name).cloned()
    }

    fn default() -> Self {
        // Fallback hardcoded registry
        let mut m = HashMap::new();
        m.insert("raylib".to_string(), "https://github.com/raysan5/raylib.git".to_string());
        m.insert("json".to_string(), "https://github.com/nlohmann/json.git".to_string());
        m.insert("fmt".to_string(), "https://github.com/fmtlib/fmt.git".to_string());
        Self(m)
    }

    fn load() -> Result<Self> {
        let cache_path = Self::get_cache_path()?;
        
        // 1. Check Cache Validity
        if let Ok(metadata) = fs::metadata(&cache_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age < Duration::from_secs(CACHE_TTL_SECS) {
                        if let Ok(content) = fs::read_to_string(&cache_path) {
                            if let Ok(reg) = serde_json::from_str::<HashMap<String, String>>(&content) {
                                return Ok(Self(reg));
                            }
                        }
                    }
                }
            }
        }

        // 2. Fetch from Remote
        print!("{} Fetching registry... ", "⚡".yellow());
        match ureq::get(REGISTRY_URL).timeout(Duration::from_secs(5)).call() {
            Ok(response) => {
                let content = response.into_string()?;
                println!("{}", "✓".green());
                
                // Save to cache
                if let Some(parent) = cache_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&cache_path, &content)?;

                let map: HashMap<String, String> = serde_json::from_str(&content)?;
                Ok(Self(map))
            }
            Err(_) => {
                println!("{}", "Failed (Using cached/fallback)".red());
                // Try reading cache even if old
                if cache_path.exists() {
                    let content = fs::read_to_string(&cache_path)?;
                    let map: HashMap<String, String> = serde_json::from_str(&content)?;
                    Ok(Self(map))
                } else {
                    Ok(Self::default())
                }
            }
        }
    }

    fn get_cache_path() -> Result<PathBuf> {
        let home = dirs::home_dir().expect("Could not find home directory");
        Ok(home.join(".cx").join(CACHE_FILE))
    }
}

pub fn resolve_alias(name: &str) -> Option<String> {
    Registry::get(name)
}
