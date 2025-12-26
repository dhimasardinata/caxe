//! Code statistics and metrics.
//!
//! This module provides the `cx stats` command which analyzes source files
//! and displays metrics like line counts, code/comment ratios, etc.
//!
//! ## Example Output
//!
//! ```text
//! 📊 Calculating statistics...
//! ┌────────────┬───────┐
//! │ Metric     │ Count │
//! ├────────────┼───────┤
//! │ Files      │ 12    │
//! │ Total Lines│ 1,234 │
//! │ Code       │ 987   │
//! │ Comments   │ 156   │
//! │ Blank      │ 91    │
//! └────────────┴───────┘
//! ```

use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn print_stats() -> Result<()> {
    println!("{} Calculating statistics...", "📊".cyan());

    let mut total_files = 0;
    let mut total_lines = 0;
    let mut code_lines = 0;
    let mut blank_lines = 0;
    let mut comment_lines = 0; // Basic heuristic

    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if ["cpp", "hpp", "c", "h", "cc", "cxx", "hh", "hxx"].contains(&s.as_ref()) {
                total_files += 1;
                if let Ok((lines, code, blank, comment)) = count_file_stats(path) {
                    total_lines += lines;
                    code_lines += code;
                    blank_lines += blank;
                    comment_lines += comment;
                }
            }
        }
    }

    // Modern Unicode Table
    let mut table = crate::ui::Table::new(&["Metric", "Count"]);
    table.add_row(vec!["Files".dimmed().to_string(), total_files.to_string()]);
    table.add_row(vec![
        "Total Lines".dimmed().to_string(),
        total_lines.to_string(),
    ]);
    table.add_row(vec!["Code".green().to_string(), code_lines.to_string()]);
    table.add_row(vec![
        "Comments".dimmed().to_string(),
        comment_lines.to_string(),
    ]);
    table.add_row(vec!["Blank".dimmed().to_string(), blank_lines.to_string()]);

    table.print();

    Ok(())
}

fn count_file_stats(path: &Path) -> Result<(usize, usize, usize, usize)> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    let mut blank = 0;
    let mut comment = 0;
    let mut code = 0;

    let mut in_block_comment = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            blank += 1;
            continue;
        }

        if in_block_comment {
            comment += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
        } else if trimmed.starts_with("//") {
            comment += 1;
        } else if trimmed.starts_with("/*") {
            comment += 1;
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
        } else {
            code += 1;
        }
    }

    Ok((total, code, blank, comment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_file_stats_simple() {
        // Create a temp file with known content
        let temp_dir = std::env::temp_dir().join("caxe_stats_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let temp_file = temp_dir.join("test.cpp");

        let content = r#"// Comment line
int main() {
    return 0;
}

"#;
        std::fs::write(&temp_file, content).unwrap();

        let (total, code, blank, comment) = count_file_stats(&temp_file).unwrap();
        assert_eq!(total, 5);
        assert_eq!(code, 3);
        assert_eq!(blank, 1);
        assert_eq!(comment, 1);

        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_count_file_stats_block_comment() {
        let temp_dir = std::env::temp_dir().join("caxe_stats_test2");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let temp_file = temp_dir.join("test2.cpp");

        let content = r#"/*
 * Block comment
 */
int main() {}"#;
        std::fs::write(&temp_file, content).unwrap();

        let (total, code, _blank, comment) = count_file_stats(&temp_file).unwrap();
        assert_eq!(total, 4);
        assert_eq!(code, 1);
        assert_eq!(comment, 3);

        std::fs::remove_file(&temp_file).ok();
    }
}
