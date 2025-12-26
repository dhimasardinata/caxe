//! # caxe - Modern C/C++ Project Manager
//!
//! caxe (pronounced "c-axe") is a zero-config build tool that cuts through C/C++ complexity.
//!
//! ## Features
//!
//! - **Zero Configuration**: Create and build C/C++ projects with zero setup
//! - **Automatic Toolchain Detection**: Finds MSVC, Clang, GCC automatically
//! - **Smart Dependencies**: Git-based with pinning (tag/branch/rev)
//! - **Parallel Builds**: Lock-free compilation using all CPU cores
//! - **Cross-Platform**: Windows, Linux, macOS, WebAssembly, Arduino
//!
//! ## Quick Start
//!
//! ```bash
//! # Create a new project
//! cx new myapp
//!
//! # Build and run
//! cx run
//! ```
//!
//! ## Module Organization
//!
//! - [`build`] - Core compilation engine with parallel builds
//! - [`config`] - Configuration parsing (`cx.toml`)
//! - [`deps`] - Dependency fetching and management
//! - [`toolchain`] - Compiler detection and selection
//! - [`commands`] - CLI command handlers

/// Core build system with parallel compilation.
pub mod build;

/// Global dependency cache management.
pub mod cache;

/// Code quality tools (clang-format, clang-tidy).
pub mod checker;

/// CI/CD configuration generators.
pub mod ci;

/// CLI command handlers extracted from main.
pub mod commands;

/// Configuration file parsing (`cx.toml`).
pub mod config;

/// Dependency fetching and management.
pub mod deps;

/// Documentation generation (Doxygen).
pub mod doc;

/// Docker configuration generator.
pub mod docker;

/// IDE configuration generators (VSCode).
pub mod ide;

/// Project import and scanning.
pub mod import;

/// Lockfile (`cx.lock`) management.
pub mod lock;

/// Project packaging and distribution.
pub mod package;

/// Library registry for `cx add`.
pub mod registry;

/// Code statistics and metrics.
pub mod stats;

/// Project templates (console, web, raylib, etc.).
pub mod templates;

/// Toolchain detection and installation.
pub mod toolchain;

/// Dependency tree visualization.
pub mod tree;

/// Terminal UI utilities (tables, colors).
pub mod ui;

/// Self-upgrade functionality.
pub mod upgrade;
