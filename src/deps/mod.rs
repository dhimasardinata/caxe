mod fetch;
mod manage;

pub use fetch::fetch_dependencies;
pub use manage::{add_dependency, remove_dependency, update_dependencies};
