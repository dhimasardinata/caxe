//! Dependency tree visualization.
//!
//! This module provides the `cx tree` command which displays the project's
//! dependency graph in a hierarchical, ASCII tree format.
//!
//! ## Example Output
//!
//! ```text
//! my-project v1.0.0
//! ├── raylib (tag: 5.0)
//! ├── json (tag: v3.11.2)
//! └── fmt (git: https://github.com/fmtlib/fmt)
//! ```

use crate::build::load_config;
use anyhow::Result;
use colored::*;

pub fn print_tree() -> Result<()> {
    let config = load_config()?;

    // Root
    println!(
        "{} v{}",
        config.package.name.bold().cyan(),
        config.package.version
    );

    if let Some(deps) = config.dependencies {
        let count = deps.len();
        for (i, (name, dep)) in deps.iter().enumerate() {
            let is_last = i == count - 1;
            let prefix = if is_last { "└──" } else { "├──" };

            // Determine version or type
            let info = match dep {
                crate::config::Dependency::Simple(url) => format!("{}", url.dimmed()),
                crate::config::Dependency::Complex {
                    git,
                    pkg,
                    tag,
                    branch,
                    rev,
                    ..
                } => {
                    if let Some(t) = tag {
                        format!("tag: {}", t.green())
                    } else if let Some(b) = branch {
                        format!("branch: {}", b.yellow())
                    } else if let Some(r) = rev {
                        format!("rev: {:.7}", r.dimmed())
                    } else if let Some(g) = git {
                        format!("git: {}", g.dimmed())
                    } else if let Some(p) = pkg {
                        format!("pkg: {}", p.cyan())
                    } else {
                        "unknown".dimmed().to_string()
                    }
                }
            };

            println!("{} {} ({})", prefix, name.bold(), info);

            // In a real sophisticated tree, we would recursively check lockfiles or
            // query the registry for sub-dependencies.
            // For now, Caxe is flat or only tracking top-level until we parse vendored deps properly.
            // So we stop here.
        }
    } else {
        println!("└── (no dependencies)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::Dependency;

    #[test]
    fn test_dependency_simple_format() {
        let dep = Dependency::Simple("https://github.com/raylib/raylib.git".to_string());
        match dep {
            Dependency::Simple(url) => assert!(url.contains("raylib")),
            _ => panic!("Expected Simple variant"),
        }
    }

    #[test]
    fn test_dependency_complex_with_tag() {
        let dep = Dependency::Complex {
            git: Some("https://github.com/nlohmann/json.git".to_string()),
            pkg: None,
            tag: Some("v3.11.2".to_string()),
            branch: None,
            rev: None,
            build: None,
            output: None,
        };

        match dep {
            Dependency::Complex { tag, .. } => {
                assert_eq!(tag, Some("v3.11.2".to_string()));
            }
            _ => panic!("Expected Complex variant"),
        }
    }

    #[test]
    fn test_dependency_complex_with_branch() {
        let dep = Dependency::Complex {
            git: Some("https://github.com/libsdl-org/SDL.git".to_string()),
            pkg: None,
            tag: None,
            branch: Some("SDL2".to_string()),
            rev: None,
            build: None,
            output: None,
        };

        match dep {
            Dependency::Complex { branch, .. } => {
                assert_eq!(branch, Some("SDL2".to_string()));
            }
            _ => panic!("Expected Complex variant"),
        }
    }

    #[test]
    fn test_dependency_pkg_config() {
        let dep = Dependency::Complex {
            git: None,
            pkg: Some("gtk+-3.0".to_string()),
            tag: None,
            branch: None,
            rev: None,
            build: None,
            output: None,
        };

        match dep {
            Dependency::Complex { pkg, .. } => {
                assert_eq!(pkg, Some("gtk+-3.0".to_string()));
            }
            _ => panic!("Expected Complex variant"),
        }
    }
}
