# caxe (cx) 🪓

[![CI/CD Pipeline](https://github.com/dhimasardinata/cx/actions/workflows/release.yml/badge.svg)](https://github.com/dhimasardinata/cx/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/caxe.svg)](https://crates.io/crates/caxe)

**caxe** _(pronounced "c-axe")_ is a modern project manager for C and C++ designed to **cut through the complexity** of legacy build systems.

It provides a unified workflow for scaffolding, building, testing, formatting, and managing dependencies—giving C/C++ developers the modern experience they deserve.

> **Zero Configuration. Lightning Fast. Batteries Included.**

## ✨ Features

- **⚡ Zero Config Start**: Create a Hello World C/C++ project in seconds.
- **📦 Smart Dependency Management**:
  - **Git Libraries**: Automatically downloads & builds libraries from GitHub.
  - **System Packages**: Native support for `pkg-config` (e.g., GTK, OpenSSL).
- **🎨 Code Formatting**: Built-in `cx fmt` command to keep your code clean (via `clang-format`).
- **🚀 Parallel & Incremental Builds**: Lock-free parallel compilation engine for maximum speed.
- **💾 Global Caching**: Libraries are downloaded once and shared across all projects.
- **👁️ Watch Mode**: Automatically recompiles and runs your project when you save a file.
- **🛠️ Flexible Configuration**: Custom binary names, environment variable support (`CC`, `CXX`), and build scripts.

## 📦 Installation

### Option 1: Download Binary (Recommended)

No Rust or Cargo required. Download the latest release for your OS:

- **Windows**: [Download cx-windows.exe](https://github.com/dhimasardinata/caxe/releases/latest)
- **Linux**: [Download cx-linux](https://github.com/dhimasardinata/caxe/releases/latest)
- **macOS**: [Download cx-macos](https://github.com/dhimasardinata/caxe/releases/latest)

> Add the binary to your system PATH to run it from anywhere.

### Option 2: Install via Cargo

If you are a Rust developer:

```bash
cargo install caxe
```

## 🚀 Usage

### 1. Create a new project

Start with a default console app, or use a template.

```bash
# Default (Hello World)
cx new my-game --lang cpp

# Web Server (cpp-httplib)
cx new my-server --template web

# Raylib Game Config
cx new my-game --template raylib
```

### 2. Manage Dependencies

Define dependencies in `cx.toml`. `caxe` supports both Git repositories and System Packages (`pkg-config`).

```toml
[dependencies]
# 1. Git Dependency (Auto download & link)
json = "https://github.com/nlohmann/json.git"

# 2. System Dependency (Uses pkg-config)
gtk4 = { pkg = "gtk4" }
openssl = { pkg = "openssl" }
```

### 3. Build & Run

```bash
# Compile and Run
cx run

# Run with optimizations (Release mode)
cx run --release

# Format code (Requires clang-format)
cx fmt
```

### 4. Watch mode (Auto-reload)

Coding without manually recompiling every time.

```bash
cx watch
```

## ⚙️ Configuration (`cx.toml`)

Example of a full configuration file:

```toml
[package]
name = "my-awesome-app"
version = "0.1.0"
edition = "c++20"

[build]
# Optional: Override output binary name (default is package name)
bin = "app"
# Optional: Custom flags
cflags = ["-O2", "-Wall", "-Wextra"]
libs = ["pthread", "m"]

[dependencies]
fmt = "https://github.com/fmtlib/fmt.git"
sdl2 = { pkg = "sdl2" }

[scripts]
pre_build = "echo Compiling..."
post_build = "echo Done!"
```

## 🛠️ Advanced

### Environment Variables

`caxe` respects standard environment variables for compiler selection:

```bash
# Linux/Mac
CXX=clang++-14 cx run

# Windows (PowerShell)
$env:CXX="g++"; cx run
```

### Unit Testing

Create a `tests/` directory and add `.cpp` files. `caxe` will compile and run them automatically.

```bash
cx test
```

## 📝 License

MIT
