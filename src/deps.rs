use anyhow::{Context, Result};
use colored::*;
use git2::Repository;
use std::collections::HashMap;
use std::fs;

pub fn fetch_dependencies(deps: &HashMap<String, String>) -> Result<Vec<String>> {
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let cache_dir = home_dir.join(".cx").join("cache");
    fs::create_dir_all(&cache_dir)?;

    let mut include_paths = Vec::new();

    println!("{} Checking dependencies...", "📦".blue());

    for (name, url) in deps {
        let lib_path = cache_dir.join(name);

        if !lib_path.exists() {
            println!("   {} Downloading {} (Global Cache)...", "⬇".cyan(), name);
            println!("     URL: {}", url);

            match Repository::clone(url, &lib_path) {
                Ok(_) => println!("     Done."),
                Err(e) => {
                    println!("{} Failed to download {}: {}", "x".red(), name, e);
                    continue;
                }
            }
        } else {
            println!("   {} Using cached: {}", "⚡".green(), name);
        }

        include_paths.push(format!("-I{}", lib_path.display()));
        include_paths.push(format!("-I{}/include", lib_path.display()));
        include_paths.push(format!("-I{}/src", lib_path.display()));
    }

    Ok(include_paths)
}
