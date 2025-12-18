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
