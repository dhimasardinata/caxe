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

> **Tip:** Rename the binary to `cx` (or `cx.exe`) and add it to your system PATH to run it from anywhere.

### Option 2: Install via Cargo

If you are a Rust developer:

```bash
cargo install caxe
```

## 🚀 Quick Start

### Interactive Mode

Simply run `cx new` without arguments to start the interactive wizard.

```bash
cx new
# ? What is your project name? › my-app
# ? Select a template: › console
# ? Select language: › cpp
```

### CLI Arguments Mode

You can also skip the questions by passing arguments directly:

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

- `--lang <c|cpp>` : Set project language (default: `cpp`).
- `--template <console|web|raylib>` : Choose a project template (default: `console`).

### `cx run`

Compiles and runs the project.

- `--release` : Enable optimizations (`-O3`).
- `-- <args>` : Pass arguments to your executable.

  ```bash
  cx run --release -- --my-arg 123
  ```

### `cx build`

Compiles the project without running it. Useful for checking errors.

- `--release` : Build for release (output in `build/release/`).

### `cx add <lib>`

Adds a Git dependency to `cx.toml`.

- Supports `user/repo` shortcut or full Git URL.

  ```bash
  cx add nlohmann/json
  cx add https://github.com/fmtlib/fmt.git
  ```

> _Note: For system packages (pkg-config), edit `cx.toml` manually (see below)._

### `cx remove <lib>`

Removes a dependency from `cx.toml` and stops linking it.

```bash
cx remove json
```

### `cx fmt`

Formats all source code in `src/` using `clang-format`.

> Requires `clang-format` to be installed on your system.

### `cx clean`

Removes the `build/` directory and metadata files (`compile_commands.json`). Use this if you want a fresh build from scratch.

### `cx watch`

Watches for file changes in `src/` and automatically recompiles & runs the project.

### `cx test`

Compiles and runs all `.cpp` or `.c` files found in the `tests/` directory.

### `cx info`

Displays useful diagnostic information:

- System OS & Architecture.
- Global cache directory path.
- Detected toolchains (gcc, clang, msvc, make, cmake).

---

## ⚙️ Configuration (`cx.toml`)

`caxe` uses `cx.toml` to manage the project. Here is a comprehensive example:

```toml
[package]
name = "my-awesome-app"
version = "0.1.0"
edition = "c++20" # or "c17" for C projects

[build]
# Override output binary name (default is package name)
bin = "app"
# Add custom compiler flags
cflags = ["-O2", "-Wall", "-Wextra", "-DDEBUG"]
# Link system libraries manually (e.g. -lm -lpthread)
libs = ["pthread", "m"]
# Force specific compiler (optional, defaults to auto-detect)
compiler = "clang++"

[dependencies]
# 1. Git Dependency (Auto download & link)
fmt = "https://github.com/fmtlib/fmt.git"

# 2. System Dependency (Uses pkg-config)
# Automatically adds include paths and link flags
gtk4 = { pkg = "gtk4" }
openssl = { pkg = "openssl" }

# 3. Complex Git Dependency
# Build a custom library from source with a specific command
raylib = { git = "https://github.com/raysan5/raylib.git", build = "make", output = "src/libraylib.a" }

[scripts]
# Run shell commands before or after build
pre_build = "echo Compiling..."
post_build = "echo Done!"
```

## 🛠️ Advanced

### Environment Variables

`caxe` respects standard environment variables for compiler selection. This is useful for CI/CD or switching compilers without editing `cx.toml`.

```bash
# Linux/Mac
CXX=clang++-14 cx run

# Windows (PowerShell)
$env:CXX="g++"; cx run
```

### Unit Testing

No need for complex test runners like GoogleTest for simple projects.

1. Create a `tests/` directory.
2. Add `.cpp` files (e.g., `tests/test_math.cpp`).
3. Use standard `assert` or return `0` for success.

```cpp
#include <cassert>
int main() {
    assert(1 + 1 == 2);
    return 0; // Pass
}
```

Run them all with:

```bash
cx test
```

## 📝 License

MIT
