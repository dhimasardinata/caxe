use super::core;
use anyhow::Result;
use colored::*;
use notify::{Config, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

pub fn watch() -> Result<()> {
    println!("{} Watching for changes in src/...", "👀".cyan());
    let (tx, rx) = channel();
    let config_notify = Config::default().with_poll_interval(Duration::from_secs(1));
    let mut watcher = notify::RecommendedWatcher::new(tx, config_notify)?;

    watcher.watch(Path::new("src"), RecursiveMode::Recursive)?;

    // First run
    run_and_clear();

    while let Ok(_) = rx.recv() {
        // Debounce simple
        std::thread::sleep(Duration::from_millis(100));
        while let Ok(_) = rx.try_recv() {}
        run_and_clear();
    }
    Ok(())
}

fn run_and_clear() {
    print!("\x1B[2J\x1B[1;1H");
    println!("{} File changed. Rebuilding...", "🔄".yellow());
    if let Err(e) = core::build_and_run(false, false, &[]) {
        println!("{} Error: {}", "x".red(), e);
    }
}
