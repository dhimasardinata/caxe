use caxe::config;
use caxe::import;
use caxe::lock::LockFile;
use caxe::registry;
use caxe::templates;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
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

fn bench_std_flags(c: &mut Criterion) {
    use caxe::build::utils::{get_std_flag_gcc, get_std_flag_msvc};

    c.bench_function("get_std_flag_msvc", |b| {
        b.iter(|| {
            let _ = get_std_flag_msvc(black_box("c++20"));
            let _ = get_std_flag_msvc(black_box("c++23"));
            let _ = get_std_flag_msvc(black_box("c17"));
        })
    });

    c.bench_function("get_std_flag_gcc", |b| {
        b.iter(|| {
            let _ = get_std_flag_gcc(black_box("c++20"));
            let _ = get_std_flag_gcc(black_box("c++23"));
            let _ = get_std_flag_gcc(black_box("gnu++17"));
        })
    });
}

fn bench_registry(c: &mut Criterion) {
    c.bench_function("registry_search", |b| {
        b.iter(|| {
            let _ = registry::search(black_box("ray"));
            let _ = registry::search(black_box("json"));
            let _ = registry::search(black_box("sdl"));
        })
    });

    c.bench_function("registry_resolve_alias", |b| {
        b.iter(|| {
            let _ = registry::resolve_alias(black_box("raylib"));
            let _ = registry::resolve_alias(black_box("json"));
            let _ = registry::resolve_alias(black_box("fmt"));
        })
    });
}

fn bench_templates(c: &mut Criterion) {
    c.bench_function("get_template_console", |b| {
        b.iter(|| {
            templates::get_template(black_box("myapp"), black_box("cpp"), black_box("console"))
        })
    });

    c.bench_function("get_template_sdl2", |b| {
        b.iter(|| templates::get_template(black_box("game"), black_box("cpp"), black_box("sdl2")))
    });
}

fn bench_lockfile_ops(c: &mut Criterion) {
    c.bench_function("lockfile_insert_get", |b| {
        b.iter(|| {
            let mut lock = LockFile::default();
            lock.insert(
                black_box("dep1".to_string()),
                black_box("https://github.com/user/dep1".to_string()),
                black_box("abc123".to_string()),
            );
            lock.insert(
                black_box("dep2".to_string()),
                black_box("https://github.com/user/dep2".to_string()),
                black_box("def456".to_string()),
            );
            let _ = lock.get(black_box("dep1"));
        })
    });
}

criterion_group!(
    benches,
    bench_ephemeral_config,
    bench_config_parse,
    bench_lock_parse,
    bench_scan_project,
    bench_std_flags,
    bench_registry,
    bench_templates,
    bench_lockfile_ops
);
criterion_main!(benches);
