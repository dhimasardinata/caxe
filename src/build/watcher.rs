use super::core;
use anyhow::Result;
use colored::*;
use notify::{Config, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

pub fn watch(run_tests: bool) -> Result<()> {
    println!("{} Watching for changes in src/...", "👀".cyan());
    if run_tests {
        println!("{} TDD Mode: Will run tests on change.", "🧪".magenta());
    }

    let (tx, rx) = channel();
    let config_notify = Config::default().with_poll_interval(Duration::from_secs(1));
    let mut watcher = notify::RecommendedWatcher::new(tx, config_notify)?;

    // Watch src/ AND tests/ if in test mode
    watcher.watch(Path::new("src"), RecursiveMode::Recursive)?;
    if run_tests && Path::new("tests").exists() {
        watcher.watch(Path::new("tests"), RecursiveMode::Recursive)?;
    }

    // First run
    run_and_clear(run_tests);

    while rx.recv().is_ok() {
        // Debounce simple
        std::thread::sleep(Duration::from_millis(100));
        while rx.try_recv().is_ok() {}
        run_and_clear(run_tests);
    }
    Ok(())
}

fn run_and_clear(run_tests: bool) {
    print!("\x1B[2J\x1B[1;1H");
    println!("{} File changed. Rebuilding...", "🔄".yellow());

    let result = if run_tests {
        super::test::run_tests(None)
    } else {
        core::build_and_run(false, false, false, vec![], None)
    };

    if let Err(e) = result {
        println!("{} Error: {}", "x".red(), e);
    }
}
