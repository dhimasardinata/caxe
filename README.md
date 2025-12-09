# caxe (cx) 🪓

[![CI](https://github.com/dhimasardinata/caxe/actions/workflows/ci.yml/badge.svg)](https://github.com/dhimasardinata/caxe/actions/workflows/ci.yml)
[![GitHub release (latest by date)](https://img.shields.io/github/v/release/dhimasardinata/caxe?label=latest%20version&color=green)](https://github.com/dhimasardinata/caxe/releases)
[![GitHub all releases](https://img.shields.io/github/downloads/dhimasardinata/caxe/total?color=blue&label=downloads&logo=github)](https://github.com/dhimasardinata/caxe/releases)
[![Crates.io](https://img.shields.io/crates/v/caxe.svg)](https://crates.io/crates/caxe)

**caxe** _(pronounced "c-axe")_ is a modern project manager for C and C++ designed to **cut through the complexity** of legacy build systems.

It provides a unified workflow for scaffolding, building, testing, formatting, and managing dependencies—giving C/C++ developers the modern experience they deserve.

> **Zero Configuration. Lightning Fast. Batteries Included.**

## ✨ Features

- **⚡ Zero Config Start**: Create a Hello World C/C++ project in seconds.
- **📦 Smart Dependency Management**:
  - **Git Libraries**: Auto-download from GitHub. Supports **Pinning** (Tag/Branch/Commit) for stability.
  - **System Packages**: Native support for `pkg-config` (e.g., GTK, OpenSSL).
  - **Header-Only Support**: Automatically detects libraries that don't need linking (e.g., nlohmann/json).
- **🎨 Code Formatting**: Built-in `cx fmt` command (via `clang-format`).
- **🚀 Parallel & Incremental Builds**: Lock-free parallel compilation engine for maximum speed.
- **💾 Global Caching**: Libraries are downloaded once and shared across all projects. Use `cx update` to refresh them.
- **👁️ Watch Mode**: Automatically recompiles and runs your project when you save a file.
- **🛠️ Flexible Configuration**: Custom binary names, environment variable support (`CC`, `CXX`), and build scripts.

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

### Option 3: Manual Download

Download the latest binary from [Releases](https://github.com/dhimasardinata/caxe/releases/latest) and add it to your PATH.

## 🚀 Quick Start

### Interactive Mode

Simply run `cx new` without arguments to start the wizard.

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

### `cx new <name>`

Creates a new project.

- `--lang <c|cpp>` : Set language.
- `--template <console|web|raylib>` : Choose template.

### `cx run`

Compiles and runs the project.

- `--release` : Enable optimizations (`-O3`).
- `-- <args>` : Pass arguments to your executable.

### `cx build`

Compiles the project without running it.

### `cx add <lib>`

Adds a Git dependency to `cx.toml`. Supports version pinning.

- **Alias (New!)**: `cx add raylib` (No URL needed!)
- **Standard**: `cx add nlohmann/json`
- **Tag**: `cx add nlohmann/json --tag v3.11.2`
- **Branch**: `cx add raysan5/raylib --branch master`
- **Commit**: `cx add fmtlib/fmt --rev a3b1c2d`

### `cx remove <lib>`

Removes a dependency from `cx.toml`.

### `cx update`

Updates all dependencies in your cache to the latest version (unless pinned).

### `cx fmt`

Formats all source code in `src/`. Requires `clang-format`.

### `cx clean`

Removes the `build/` directory and metadata files.

### `cx watch`

Watches for file changes and auto-runs.

### `cx test`

Compiles and runs files in `tests/` directory.

### `cx info`

Displays diagnostic information (OS, Cache Path, Compilers).

---

## ⚙️ Configuration (`cx.toml`)

Comprehensive configuration example:

```toml
[package]
name = "my-awesome-app"
version = "0.1.0"
edition = "c++20"

[build]
bin = "app" # Output: app.exe
cflags = ["-O2", "-Wall", "-Wextra"]
libs = ["pthread", "m"]

[dependencies]
# 1. Simple Git (HEAD)
fmt = "https://github.com/fmtlib/fmt.git"

# 2. Pinned Version (Recommended for production)
json = { git = "https://github.com/nlohmann/json.git", tag = "v3.11.2" }
# Pin to specific commit hash
utils = { git = "...", rev = "a1b2c3d4" }

# 3. System Dependency (pkg-config)
gtk4 = { pkg = "gtk4" }

# 4. Complex Build (Library with source code)
# 'output' field tells caxe to link this file.
# If 'output' is missing, caxe treats it as Header-Only.
raylib = { git = "...", build = "make", output = "src/libraylib.a" }

[scripts]
pre_build = "echo Compiling..."
post_build = "echo Done!"
```

## 🛠️ Advanced

### Header-Only Libraries

`caxe` is smart. If you add a library like `nlohmann/json` or `fmt` and do not specify an `output` file in `cx.toml`, `caxe` assumes it is a **Header-Only** library. It will add the include paths (`-I`) but will not attempt to link any static library.

### Environment Variables

Override the compiler without changing config:

```bash
# Windows (PowerShell)
$env:CXX="g++"; cx run
```

### Unit Testing

Create a `tests/` directory and add `.cpp` files.

```bash
cx test
```

## 📝 License

MIT
