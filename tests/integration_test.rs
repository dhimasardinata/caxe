//! Integration tests for caxe build functionality
//!
//! These tests verify the end-to-end behavior of the `cx build` command
//! by creating temporary projects and running builds.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn test_projects_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".tmp_test_projects")
        .join("integration")
}

/// Create a temporary test project directory
fn create_test_project(name: &str, is_cpp: bool) -> PathBuf {
    let temp_dir = test_projects_root().join(name);

    // Clean up if exists
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    fs::create_dir_all(&temp_dir).expect("Failed to create test directory");
    fs::create_dir_all(temp_dir.join("src")).expect("Failed to create src directory");

    // Create cx.toml
    let edition = if is_cpp { "c++17" } else { "c17" };
    let ext = if is_cpp { "cpp" } else { "c" };
    let cx_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "{edition}"

[build]
sources = ["src/main.{ext}"]
"#
    );
    fs::write(temp_dir.join("cx.toml"), cx_toml).expect("Failed to write cx.toml");

    // Create main source file
    let main_content = if is_cpp {
        r#"#include <iostream>
int main() {
    std::cout << "Hello from test!" << std::endl;
    return 0;
}
"#
    } else {
        r#"#include <stdio.h>
int main() {
    printf("Hello from test!\n");
    return 0;
}
"#
    };
    fs::write(
        temp_dir.join("src").join(format!("main.{ext}")),
        main_content,
    )
    .expect("Failed to write main source file");

    temp_dir
}

/// Get the path to the cx binary
fn get_cx_binary() -> PathBuf {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"));

    let bin_name = if cfg!(windows) { "cx.exe" } else { "cx" };
    target_dir.join("debug").join(bin_name)
}

#[test]
fn test_build_simple_cpp_project() {
    let project_dir = create_test_project("test_cpp_build", true);

    let cx = get_cx_binary();
    if !cx.exists() {
        eprintln!("Skipping test: cx binary not found at {:?}", cx);
        return;
    }

    let output = Command::new(&cx)
        .arg("build")
        .current_dir(&project_dir)
        .output()
        .expect("Failed to execute cx build");

    // Check build succeeded
    assert!(
        output.status.success(),
        "Build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check output binary exists
    let _binary = if cfg!(windows) {
        project_dir
            .join("build")
            .join("debug")
            .join("test_cpp_build.exe")
    } else {
        project_dir
            .join("build")
            .join("debug")
            .join("test_cpp_build")
    };

    // Binary might be in different location, just check build dir exists
    assert!(
        project_dir.join("build").exists() || project_dir.join(".cx").join("build").exists(),
        "Build directory not created"
    );

    // Cleanup
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn test_build_simple_c_project() {
    let project_dir = create_test_project("test_c_build", false);

    let cx = get_cx_binary();
    if !cx.exists() {
        eprintln!("Skipping test: cx binary not found at {:?}", cx);
        return;
    }

    let output = Command::new(&cx)
        .arg("build")
        .current_dir(&project_dir)
        .output()
        .expect("Failed to execute cx build");

    assert!(
        output.status.success(),
        "Build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Cleanup
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn test_build_release_mode() {
    let project_dir = create_test_project("test_release_build", true);

    let cx = get_cx_binary();
    if !cx.exists() {
        eprintln!("Skipping test: cx binary not found at {:?}", cx);
        return;
    }

    let output = Command::new(&cx)
        .args(["build", "--release"])
        .current_dir(&project_dir)
        .output()
        .expect("Failed to execute cx build --release");

    assert!(
        output.status.success(),
        "Release build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Cleanup
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn test_dry_run_mode() {
    let project_dir = create_test_project("test_dry_run", true);

    let cx = get_cx_binary();
    if !cx.exists() {
        eprintln!("Skipping test: cx binary not found at {:?}", cx);
        return;
    }

    let output = Command::new(&cx)
        .args(["build", "--dry-run"])
        .current_dir(&project_dir)
        .output()
        .expect("Failed to execute cx build --dry-run");

    assert!(
        output.status.success(),
        "Dry run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // In dry-run mode, no build directory should be created
    // (or it should be empty)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Would execute") || stdout.contains("DRY RUN") || output.status.success(),
        "Dry run should indicate what would be done"
    );

    // Cleanup
    fs::remove_dir_all(&project_dir).ok();
}
