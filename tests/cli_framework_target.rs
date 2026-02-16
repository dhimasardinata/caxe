//! Integration tests for framework and target CLI behavior.
//!
//! Focuses on v0.3.9 hardening around:
//! - Framework support-status UX and mutation safety
//! - Deferred target mutation commands and guidance

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}

fn test_project_dir(name: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".tmp_test_projects")
        .join(name)
}

fn create_basic_project(name: &str, framework: Option<&str>) -> PathBuf {
    let temp_dir = test_project_dir(name);

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    fs::create_dir_all(temp_dir.join("src")).expect("Failed to create test project dirs");

    let framework_line = framework
        .map(|value| format!("framework = \"{value}\"\n"))
        .unwrap_or_default();

    let cx_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "c++17"

[build]
sources = ["src/main.cpp"]
{framework_line}"#
    );
    fs::write(temp_dir.join("cx.toml"), cx_toml).expect("Failed to write cx.toml");

    let main_cpp = r#"#include <iostream>
int main() {
    std::cout << "ok" << std::endl;
    return 0;
}
"#;
    fs::write(temp_dir.join("src").join("main.cpp"), main_cpp).expect("Failed to write source");

    temp_dir
}

fn get_cx_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("Failed to locate current test exe");
    path.pop();
    path.pop();
    if cfg!(windows) {
        path.join("cx.exe")
    } else {
        path.join("cx")
    }
}

fn run_cx(project_dir: &Path, args: &[&str]) -> Output {
    let cx = get_cx_binary();
    if !cx.exists() {
        panic!("cx binary not found at {:?}", cx);
    }

    Command::new(cx)
        .args(args)
        .current_dir(project_dir)
        .output()
        .expect("Failed to run cx")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn framework_add_dependency_alias_fails_with_guidance() {
    let name = unique_name("framework-add-fmt");
    let project_dir = create_basic_project(&name, None);

    let output = run_cx(&project_dir, &["framework", "add", "fmt"]);
    let text = output_text(&output);

    assert!(
        !output.status.success(),
        "framework add fmt should fail.\n{}",
        text
    );
    assert!(
        text.contains("cx add fmt"),
        "Expected migration guidance to cx add fmt.\n{}",
        text
    );

    let cx_toml = fs::read_to_string(project_dir.join("cx.toml")).expect("Failed to read cx.toml");
    assert!(
        !cx_toml.contains("framework = \"fmt\""),
        "Unsupported framework add must not mutate [build].framework.\n{}",
        cx_toml
    );

    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn framework_add_daxe_updates_build_framework() {
    let name = unique_name("framework-add-daxe");
    let project_dir = create_basic_project(&name, None);

    let output = run_cx(&project_dir, &["framework", "add", "daxe"]);
    let text = output_text(&output);

    assert!(
        output.status.success(),
        "framework add daxe should succeed.\n{}",
        text
    );

    let cx_toml = fs::read_to_string(project_dir.join("cx.toml")).expect("Failed to read cx.toml");
    assert!(
        cx_toml.contains("[build]"),
        "cx.toml should keep [build] section.\n{}",
        cx_toml
    );
    assert!(
        cx_toml.contains("framework = \"daxe\""),
        "Integrated framework add should set [build].framework.\n{}",
        cx_toml
    );

    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn target_add_is_deferred_with_non_zero_and_profile_guidance() {
    let name = unique_name("target-add-deferred");
    let project_dir = create_basic_project(&name, None);

    let output = run_cx(&project_dir, &["target", "add", "esp32"]);
    let text = output_text(&output);

    assert!(
        !output.status.success(),
        "target add should fail non-zero while deferred.\n{}",
        text
    );
    assert!(
        text.contains("deferred in v0.3.x patch releases"),
        "Expected deferred-status messaging.\n{}",
        text
    );
    assert!(
        text.contains("cx build --profile <name>"),
        "Expected profile-first guidance.\n{}",
        text
    );

    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn target_help_marks_mutations_as_deferred() {
    let name = unique_name("target-help");
    let project_dir = create_basic_project(&name, None);

    let output = run_cx(&project_dir, &["target", "--help"]);
    let text = output_text(&output);

    assert!(
        output.status.success(),
        "target --help should succeed.\n{}",
        text
    );
    assert!(
        text.contains("deferred in v0.3.x; use profiles"),
        "Help output should mark mutation commands as deferred.\n{}",
        text
    );

    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn build_with_framework_alias_warns_but_succeeds_in_dry_run() {
    let name = unique_name("framework-alias-build");
    let project_dir = create_basic_project(&name, Some("fmt"));

    let output = run_cx(&project_dir, &["build", "--dry-run"]);
    let text = output_text(&output);

    assert!(
        output.status.success(),
        "build --dry-run should stay non-fatal for framework aliases.\n{}",
        text
    );
    assert!(
        text.contains("dependency-alias"),
        "Expected alias warning in output.\n{}",
        text
    );
    assert!(
        text.contains("cx add fmt"),
        "Expected explicit migration hint.\n{}",
        text
    );

    fs::remove_dir_all(&project_dir).ok();
}
