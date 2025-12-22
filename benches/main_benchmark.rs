use caxe::config;
use caxe::import;
use caxe::lock::LockFile;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use toml;

const MOCK_CONFIG: &str = r#"
[package]
name = "benchmark_project"
version = "0.1.0"
edition = "c++20"

[build]
compiler = "clang++"
bin = "bench_app"
sources = ["src/main.cpp", "src/utils.cpp"]
"#;

const MOCK_LOCK: &str = r#"
[package]
"dep1" = { git = "https://github.com/user/dep1", rev = "abcdef1234567890" }
"dep2" = { git = "https://github.com/user/dep2", rev = "1234567890abcdef" }
"#;

fn bench_ephemeral_config(c: &mut Criterion) {
    c.bench_function("create_ephemeral_config", |b| {
        b.iter(|| {
            config::create_ephemeral_config(
                black_box("myscript"),
                black_box("myscript"),
                black_box("auto"),
                black_box(true),
            )
        })
    });
}

fn bench_config_parse(c: &mut Criterion) {
    c.bench_function("parse_cx_toml", |b| {
        b.iter(|| {
            let _: config::CxConfig = toml::from_str(black_box(MOCK_CONFIG)).unwrap();
        })
    });
}

fn bench_lock_parse(c: &mut Criterion) {
    c.bench_function("parse_cx_lock", |b| {
        b.iter(|| {
            let _: LockFile = toml::from_str(black_box(MOCK_LOCK)).unwrap();
        })
    });
}

fn bench_scan_project(c: &mut Criterion) {
    // Setup a temp dir for scanning
    let temp_dir = std::env::temp_dir().join("caxe_bench_scan");
    if !temp_dir.exists() {
        std::fs::create_dir_all(&temp_dir.join("src")).unwrap();
        std::fs::write(temp_dir.join("src/main.cpp"), "int main() { return 0; }").unwrap();
    }

    c.bench_function("scan_project_simple", |b| {
        b.iter(|| import::scan_project(black_box(&temp_dir)).unwrap())
    });
}

criterion_group!(
    benches,
    bench_ephemeral_config,
    bench_config_parse,
    bench_lock_parse,
    bench_scan_project
);
criterion_main!(benches);
