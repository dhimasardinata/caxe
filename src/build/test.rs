use super::utils::{get_compiler, load_config};
use crate::config::CxConfig;
use crate::deps;
use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

pub fn run_tests() -> Result<()> {
    // Load config or default
    let config = load_config().unwrap_or_else(|_| CxConfig {
        package: crate::config::PackageConfig {
            name: "test_runner".into(),
            version: "0.0.0".into(),
            edition: "c++20".into(),
        },
        ..Default::default()
    });

    let test_dir_str = config
        .test
        .as_ref()
        .and_then(|t| t.source_dir.clone())
        .unwrap_or_else(|| "tests".to_string());
    let test_dir = Path::new(&test_dir_str);

    if !test_dir.exists() {
        println!("{} No {}/ directory found.", "!".yellow(), test_dir_str);
        return Ok(());
    }

    let mut include_paths = Vec::new();
    let mut extra_cflags = Vec::new();
    let mut dep_libs = Vec::new();

    if let Some(deps) = &config.dependencies {
        if !deps.is_empty() {
            let (paths, cflags, libs) = deps::fetch_dependencies(deps)?;
            include_paths = paths;
            extra_cflags = cflags;
            dep_libs = libs;
        }
    }

    println!("{} Running tests...", "🧪".magenta());
    fs::create_dir_all("build/tests")?;

    let mut total_tests = 0;
    let mut passed_tests = 0;

    for entry in WalkDir::new(test_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_cpp = path.extension().map_or(false, |ext| {
            ["cpp", "cc", "cxx"].contains(&ext.to_str().unwrap())
        });
        let is_c = path.extension().map_or(false, |ext| ext == "c");

        if is_cpp || is_c {
            total_tests += 1;
            let test_name = path.file_stem().unwrap().to_string_lossy();
            let output_bin = format!("build/tests/{}", test_name);

            print!("   TEST {} ... ", test_name.bold());

            let compiler = get_compiler(&config, is_cpp);
            let is_msvc = compiler.contains("cl.exe") || compiler == "cl";
            let mut cmd = Command::new(&compiler);

            if is_msvc {
                cmd.arg("/nologo");
                cmd.arg("/EHsc");
                cmd.arg(path);
                cmd.arg(format!("/Fe{}", output_bin)); // Output exe name
                cmd.arg(format!("/std:{}", config.package.edition));

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("/I{}", p.display()));
                }
            } else {
                cmd.arg(path);
                cmd.arg("-o").arg(&output_bin);
                cmd.arg(format!("-std={}", config.package.edition));

                // Includes
                for p in &include_paths {
                    cmd.arg(format!("-I{}", p.display()));
                }
            }

            cmd.args(&extra_cflags);

            if let Some(build_cfg) = &config.build {
                if let Some(flags) = &build_cfg.cflags {
                    cmd.args(flags);
                }
            }

            // Link Libs
            if is_msvc {
                cmd.arg("/link");
            }
            cmd.args(&dep_libs);

            if let Some(build_cfg) = &config.build {
                if let Some(libs) = &build_cfg.libs {
                    for lib in libs {
                        if is_msvc {
                            cmd.arg(format!("{}.lib", lib));
                        } else {
                            cmd.arg(format!("-l{}", lib));
                        }
                    }
                }
            }

            let output = cmd.output();
            if output.is_err() || !output.as_ref().unwrap().status.success() {
                println!("{}", "COMPILE FAIL".red());
                if let Ok(out) = output {
                    println!("{}", String::from_utf8_lossy(&out.stderr));
                }
                continue;
            }

            let run_path = if cfg!(target_os = "windows") {
                format!("{}.exe", output_bin)
            } else {
                format!("./{}", output_bin)
            };
            let run_status = Command::new(&run_path).status();

            match run_status {
                Ok(status) => {
                    if status.success() {
                        println!("{}", "PASS".green());
                        passed_tests += 1;
                    } else {
                        println!("{}", "FAIL".red());
                    }
                }
                Err(_) => println!("{}", "EXEC FAIL".red()),
            }
        }
    }

    println!("\nTest Result: {}/{} passed.", passed_tests, total_tests);
    if total_tests > 0 && passed_tests == total_tests {
        println!("{}", "ALL TESTS PASSED ✨".green().bold());
    } else if total_tests > 0 {
        println!("{}", "SOME TESTS FAILED 💀".red().bold());
    }

    Ok(())
}
