# cx - The Modern C/C++ Project Manager 🚀

**cx** is a blazingly fast project manager and build tool for C and C++, written in Rust. It aims to provide a modern developer experience similar to `cargo` (Rust) or `npm` (JS) but for C++.

> Built for speed, simplicity, and ease of use.

## ✨ Features

- **⚡ Zero Config Start**: Create a Hello World C++ project in seconds.
- **📦 Smart Dependency Management**: define dependencies in `cx.toml`. `cx` automatically downloads libraries from Git and handles linking.
- **💾 Global Caching**: Libraries are downloaded once and shared across all projects (saves disk space & bandwidth).
- **👁️ Watch Mode**: Automatically recompiles and runs your project when you save a file (`cx watch`).
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

```bash
cx new my-game --lang cpp
cd my-game
```

### 2. Run the project

```bash
cx run
# Or run with optimizations
cx run --release
```

### 3. Watch mode (Auto-reload)

Coding without manually recompiling every time.

```bash
cx watch
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

## 📝 License

MIT
