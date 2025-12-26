//! Core build system with parallel compilation.
//!
//! This module contains the heart of caxe: the compilation engine that transforms
//! C/C++ source files into executables with lock-free parallelism.
//!
//! ## Features
//!
//! - **Parallel compilation**: Uses rayon for lock-free, multi-core builds
//! - **Incremental builds**: Only recompiles changed files
//! - **Arduino support**: Build and upload Arduino sketches
//! - **File watching**: Auto-rebuild on file save
//! - **Test runner**: Compile and run unit tests
//!
//! ## Submodules
//!
//! - [`core`] - Main build logic and parallel compilation
//! - [`utils`] - Toolchain detection and helper functions
//! - [`test`] - Test runner for C/C++ unit tests
//! - [`arduino`] - Arduino/IoT build support

pub mod arduino;
mod clean;
mod core;
mod feedback;
mod test;
pub mod utils;
mod watcher;

pub use clean::clean;
pub use core::{BuildOptions, build_and_run, build_project};
pub use test::run_tests;
pub use utils::load_config;
pub use watcher::watch;
