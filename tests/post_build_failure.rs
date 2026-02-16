//! Post-build script failure detection tests
//!
//! These tests verify that post-build script failures are properly propagated
//! as build failures, not silently ignored.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn test_project_dir(name: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".tmp_test_projects")
        .join(name)
}

fn create_project_with_failing_post_build(name: &str) -> PathBuf {
    let temp_dir = test_project_dir(name);

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("src")).unwrap();

    let failing_cmd = if cfg!(windows) {
        "cmd /c exit 1"
    } else {
        "false"
    };

    let cx_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "c++17"

[build]
sources = ["src/main.cpp"]

[scripts]
post_build = "{failing_cmd}"
"#
    );
    fs::write(temp_dir.join("cx.toml"), cx_toml).unwrap();

    let main_content = r#"#include <iostream>
int main() {
    std::cout << "Build succeeded!" << std::endl;
    return 0;
}
"#;
    fs::write(temp_dir.join("src").join("main.cpp"), main_content).unwrap();

    temp_dir
}

fn create_project_with_succeeding_post_build(name: &str) -> PathBuf {
    let temp_dir = test_project_dir(name);

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("src")).unwrap();

    let succeeding_cmd = if cfg!(windows) {
        "cmd /c echo Post-build success"
    } else {
        "echo Post-build success"
    };

    let cx_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "c++17"

[build]
sources = ["src/main.cpp"]

[scripts]
post_build = "{succeeding_cmd}"
"#
    );
    fs::write(temp_dir.join("cx.toml"), cx_toml).unwrap();

    let main_content = r#"#include <iostream>
int main() {
    std::cout << "Build succeeded!" << std::endl;
    return 0;
}
"#;
    fs::write(temp_dir.join("src").join("main.cpp"), main_content).unwrap();

    temp_dir
}

fn get_cx_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    if cfg!(windows) {
        path.join("cx.exe")
    } else {
        path.join("cx")
    }
}

#[test]
fn test_post_build_failure_causes_build_failure() {
    let project_dir = create_project_with_failing_post_build("test_postbuild_fail");

    let cx = get_cx_binary();
    if !cx.exists() {
        eprintln!("Skipping: cx binary not found at {:?}", cx);
        return;
    }

    let output = Command::new(&cx)
        .arg("build")
        .current_dir(&project_dir)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Post-build script failed") || stderr.contains("Post-build script failed"),
        "Should mention post-build failure.\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    assert!(
        !output.status.success(),
        "Build should fail when post_build fails.\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn test_post_build_success_allows_build_success() {
    let project_dir = create_project_with_succeeding_post_build("test_postbuild_success");

    let cx = get_cx_binary();
    if !cx.exists() {
        eprintln!("Skipping: cx binary not found at {:?}", cx);
        return;
    }

    let output = Command::new(&cx)
        .arg("build")
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Build should succeed when post_build succeeds.\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&project_dir).ok();
}
