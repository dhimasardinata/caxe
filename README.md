# cx - The Modern C/C++ Project Manager 🚀

[![CI/CD Pipeline](https://github.com/DhimasArdinata/cx/actions/workflows/ci.yml/badge.svg)](https://github.com/DhimasArdinata/cx/actions/workflows/ci.yml)

**cx** is a blazingly fast project manager and build tool for C and C++, written in Rust. It aims to provide a modern developer experience similar to `cargo` (Rust) or `npm` (JS) but for C++.

> Built for speed, simplicity, and ease of use.

## ✨ Features

- **⚡ Zero Config Start**: Create a Hello World C++ project in seconds.
- **📑 Project Templates**: Start quickly with presets for Raylib or Web Servers.
- **📦 Smart Dependency Management**: Define dependencies in `cx.toml` or use `cx add`. `cx` automatically downloads libraries from Git and handles linking.
- **💾 Global Caching**: Libraries are downloaded once and shared across all projects.
- **👁️ Watch Mode**: Automatically recompiles and runs your project when you save a file.
- **🧪 Built-in Testing**: Run unit tests easily without configuring external frameworks.
- **🚀 Incremental Builds**: Only recompiles changed files.
- **🛠️ Custom Configuration**: Support for C++17/20, custom compiler flags, and system linking.

## 📦 Installation

Prerequisites:

- Rust (Cargo)
- Clang or GCC installed

```bash
git clone https://github.com/DhimasArdinata/cx.git
cd cx
cargo install --path .
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

Add or remove libraries directly from the CLI.

```bash
# Add a library (supports 'user/repo' or full git URL)
cx add fmtlib/fmt
cx add nlohmann/json

# Remove a library
cx remove fmt
```

### 3. Run the project

```bash
cx run
# Or run with optimizations
cx run --release
```

### 4. Watch mode (Auto-reload)

Coding without manually recompiling every time.

```bash
cx watch
```

### 5. Unit Testing 🧪

No need for complex test runners like GoogleTest or Catch2 for simple projects.

1. Create a `tests/` directory in your project root.
2. Add `.cpp` files (e.g., `tests/test_math.cpp`).
3. Use standard `assert` or return `0` for success.

```cpp
#include <cassert>

int main() {
    int x = 10;
    assert(x + 5 == 15); // If this fails, the test fails
    return 0;
}
```

Run the tests:

```bash
cx test
```

## ⚙️ Configuration (`cx.toml`)

No more confusing Makefiles or CMakeLists.

```toml
[package]
name = "my-game"
version = "0.1.0"
edition = "c++20"

[build]
cflags = ["-O2", "-Wall"]
libs = ["pthread", "m"] # Link system libraries

[dependencies]
# Dependencies are automatically fetched & linked!
json = "https://github.com/nlohmann/json.git"
fmt = "https://github.com/fmtlib/fmt.git"
```

## 🖼️ Showcase

Experience a modern development workflow. Here is an example of running a C++ Web Server (`cpp-httplib`) with zero manual configuration.

### 1. The Build Process

`cx` handles dependency fetching, caching, and compiling automatically.

![Terminal Build Demo](assets/demo-terminal.png)

### 2. The Result

The C++ server is up and running instantly.

![Browser Output Demo](assets/demo-browser.png)

## 📝 License

MIT
