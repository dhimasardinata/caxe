//! Dependency fetching and management.
//!
//! This module handles all dependency-related operations including:
//!
//! - **Fetching**: Download dependencies from Git repositories
//! - **Management**: Add, remove, and update dependencies in `cx.toml`
//! - **Vendoring**: Copy dependencies locally for offline builds
//!
//! ## Commands
//!
//! - `cx add <lib>` - Add a library from registry or Git URL
//! - `cx remove <lib>` - Remove a dependency
//! - `cx update` - Update all dependencies to latest versions
//! - `cx vendor` - Copy dependencies into `vendor/` directory

mod fetch;
mod manage;
mod vendor;

pub use fetch::{ModuleFile, fetch_dependencies};
pub use manage::{add_dependency, remove_dependency, update_dependencies};
pub use vendor::vendor_dependencies;
