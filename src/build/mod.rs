mod clean;
mod core;
mod feedback;
mod test;
mod utils;
mod watcher;

pub use clean::clean;
pub use core::{build_and_run, build_project};
pub use test::run_tests;
pub use utils::load_config;
pub use watcher::watch;
