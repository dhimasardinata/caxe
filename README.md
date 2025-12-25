# caxe (cx) 🪓

[![CI](https://github.com/dhimasardinata/caxe/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/dhimasardinata/caxe/actions/workflows/ci.yml)
[![GitHub release (latest by date)](https://img.shields.io/github/v/release/dhimasardinata/caxe?label=latest%20version&color=green)](https://github.com/dhimasardinata/caxe/releases)
[![GitHub all releases](https://img.shields.io/github/downloads/dhimasardinata/caxe/total?color=blue&label=downloads&logo=github)](https://github.com/dhimasardinata/caxe/releases)
[![Crates.io](https://img.shields.io/crates/v/caxe.svg)](https://crates.io/crates/caxe)

**caxe** _(pronounced "c-axe")_ is a modern project manager for C and C++ designed to **cut through the complexity** of legacy build systems.

It provides a unified workflow for scaffolding, building, testing, formatting, and managing dependencies—giving C/C++ developers the modern experience they deserve.

> **Zero Configuration. Lightning Fast. Batteries Included.**

## ✨ Features

- **⚡ Zero Config Start**: Create a Hello World C/C++ project in seconds.
- **🔧 Automatic Toolchain Discovery**: Detects MSVC, Clang-CL, Clang++, and GCC without relying on PATH. Uses `vswhere` on Windows.
- **📦 Smart Dependency Management**:
  - **Git Libraries**: Auto-download from GitHub. Supports **Pinning** (Tag/Branch/Commit) for stability.
  - **System Packages**: Native support for `pkg-config` (e.g., GTK, OpenSSL).
  - **Vendor Mode**: `cx vendor` to copy dependencies locally for offline builds.
- **🚀 High-Performance Builds**: 
  - **Lock-free Parallel Compilation**: Utilizes all CPU cores.
  - **Caching**: **CCache** integration, incremental builds, and PCH support.
  - **LTO**: Link Time Optimization for release builds.
- **🧪 Smart Testing**: 
  - Auto-links project sources for unit testing internals.
  - Test filtering (`--filter`) and binary caching.
- **📊 Insights**: `cx stats` for code metrics and `cx tree` for dependency graphs.
- **🌍 WebAssembly**: `cx build --wasm` (via Emscripten) support out of the box.
- **🤖 Arduino/IoT**: Auto-detect `.ino` files, build and upload via `arduino-cli`.
- **🎯 Cross-Platform Targets**: Manage build targets (Windows, Linux, macOS, WebAssembly, ESP32).
- **🛡️ Safety**: `cx build --sanitize` for Address/Undefined Behavior sanitizers.
- **🎨 Code Formatting**: Built-in `cx fmt` command (via `clang-format`) with `--check` for CI.
- **🎯 Build Profiles**: Custom profiles with inheritance for cross-compilation (`--profile esp32`).
- **🤖 Automation**: Generators for **Docker**, **GitHub Actions**, and **VSCode** configs.

## 📦 Installation

### Automatic Script (Recommended)

**Windows (PowerShell)**:
```powershell
iwr https://raw.githubusercontent.com/dhimasardinata/caxe/main/install.ps1 -useb | iex
```

**Unix (Linux/macOS)**:
```bash
curl -fsSL https://raw.githubusercontent.com/dhimasardinata/caxe/main/install.sh | sh
```

### Option 2: Install via Cargo

```bash
cargo install caxe
```

## 🚀 Quick Start

### Interactive Mode

Simply run `cx` or `cx new` without given name to start the wizard.

```bash
cx new
# ? What is your project name? › my-app
# ? Select a template: › console
# ? Select language: › cpp
```

### CLI Arguments Mode

```bash
# Default (Hello World)
cx new my-game --lang cpp

# Web Server (cpp-httplib)
cx new my-server --template web

# Raylib Game Config
cx new my-game --template raylib
```

---

## 📖 CLI Reference

### Project Management
- **`cx new <name>`**: Create a new project.
- **`cx init`**: Initialize `cx.toml` in an existing directory (imports CMake/Makefile projects!).
- **`cx info`**: Show system, cache, and toolchain info.
- **`cx doctor`**: Diagnose system issues (missing tools, compilers).
- **`cx stats`**: Show project code metrics (LOC, files).

### Build & Run
- **`cx run`**: Build and run the project.
- **`cx build`**: Compile only.
  - `--release`: Optimize for speed (`-O3` / `/O2`).
  - `--profile <name>`: Use a named profile (e.g., `--profile esp32`).
  - `--wasm`: Compile to WebAssembly (requires Emscripten).
  - `--lto`: Enable Link Time Optimization.
  - `--sanitize=<check>`: Enable runtime sanitizers (e.g., `address`, `undefined`).
  - `--trace`: Generate build trace (`.cx/build/build_trace.json` for Chrome Tracing).
- **`cx watch`**: Rebuild on file save.
- **`cx clean`**: Remove build artifacts.
- **`cx package`**: Create a distribution archive (ZIP) containing the executable, DLLs, and assets.

### Arduino/IoT
- **`cx build --arduino`**: Build Arduino sketch (auto-detected if `.ino` files present).
- **`cx upload -p COM3`**: Upload sketch to Arduino board.
- **`cx new myproject --template arduino`**: Create Arduino project.

### Cross-Platform
- **`cx target list`**: Show available cross-compilation targets.
- **`cx target add <name>`**: Add a target to your project.
- **`cx target remove <name>`**: Remove a target.
- **`cx generate cmake`**: Generate CMakeLists.txt from cx.toml.
- **`cx generate ninja`**: Generate build.ninja from cx.toml.

### Dependencies
- **`cx add <lib>`**: Add a library from registry or Git URL.
- **`cx remove <lib>`**: Remove a dependency.
- **`cx update`**: Update dependencies to latest versions.
- **`cx vendor`**: Copy all dependencies into `vendor/` for commit/offline use.
- **`cx lock`**: Manage `cx.lock` file (`--check`, `--update`).
- **`cx sync`**: Synchronize dependencies with `cx.lock` to ensure reproducible builds.
- **`cx tree`**: Visualize the dependency graph.

### Testing & Quality
- **`cx test`**: Run unit tests in `tests/`.
  - `--filter <name>`: Run specific tests.
- **`cx fmt`**: Format code with `clang-format`.
  - `--check`: Verify formatting without modifying (for CI).
- **`cx check`**: Static analysis (clang-tidy/cppcheck).

### Ecosystem
- **`cx toolchain`**: Manage C/C++ compilers.
  - `list`: Show detected compilers.
  - `select`: Choose active compiler interactively.
  - `install`: Interactive wizard to install toolchains and dev tools.
  - `update`: Check for and install toolchain updates.
- **`cx docker`**: Generate a Dockerfile.
- **`cx ci`**: Generate a GitHub Actions workflow.
- **`cx setup-ide`**: Generate VSCode configuration (`.vscode/`).

## ⚙️ Configuration (`cx.toml`)

```toml
[package]
name = "my-awesome-app"
version = "0.1.0"
edition = "c++20"

[build]
bin = "app" # Output: app.exe
compiler = "clang"  # Options: msvc, clang, clang-cl, g++
flags = ["-O2", "-Wall", "-Wextra"]
libs = ["pthread", "m"]
pch = "src/pch.hpp" # Precompiled Header (Optional)

[dependencies]
# 1. Simple Git (HEAD)
fmt = "https://github.com/fmtlib/fmt.git"

# 2. Pinned Version (Recommended for production)
json = { git = "https://github.com/nlohmann/json.git", tag = "v3.11.2" }

# 3. System Dependency (pkg-config)
gtk4 = { pkg = "gtk4" }

# Build Profiles (for cross-compilation)
[profile:esp32]
base = "release"  # Inherit from release
compiler = "xtensa-esp32-elf-g++"
flags = ["-mcpu=esp32", "-ffunction-sections"]

[arduino]
board = "arduino:avr:uno"  # or "esp32:esp32:esp32"
port = "COM3"              # optional, for upload
```

## 📝 License

MIT
