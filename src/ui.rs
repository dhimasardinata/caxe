//! Terminal UI utilities.
//!
//! This module provides UI components for CLI output, including a responsive
//! table with Unicode box-drawing characters.
//!
//! ## Components
//!
//! - `Table` - Auto-sizing table with headers and rows
//!
//! ## Example
//!
//! ```rust
//! let mut table = Table::new(&["Name", "Value"]);
//! table.add_row(vec!["key".to_string(), "value".to_string()]);
//! table.print();
//! ```

use colored::*;
use std::cmp;

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: &[&str]) -> Self {
        Self {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        if row.len() == self.headers.len() {
            self.rows.push(row);
        }
    }

    pub fn print(&self) {
        if self.headers.is_empty() {
            return;
        }

        // Get terminal width
        let term = console::Term::stdout();
        let (_term_height, term_width) = term.size();
        let max_width = term_width as usize;

        // Calculate initial max content widths per column
        let mut col_widths = vec![0; self.headers.len()];

        // Header widths
        for (i, header) in self.headers.iter().enumerate() {
            col_widths[i] = cmp::max(col_widths[i], header.chars().count());
        }

        // Row widths (sanitized)
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                let clean = sanitize_content(cell);
                let clean_len = strip_ansi(&clean).chars().count();
                col_widths[i] = cmp::max(col_widths[i], clean_len);
            }
        }

        let overhead = 3 + 3 * self.headers.len();
        let total_content_width: usize = col_widths.iter().sum();
        let total_required = overhead + total_content_width;

        if total_required > max_width {
            let available_content_width = max_width.saturating_sub(overhead);

            let mut current_width = total_content_width;

            // Loop until fit
            let mut changed = true;
            while current_width > available_content_width && changed {
                changed = false;
                // Find widest column that is > 8
                let mut max_idx = 0;
                let mut max_val = 0;
                for (i, &w) in col_widths.iter().enumerate() {
                    if w > max_val {
                        max_val = w;
                        max_idx = i;
                    }
                }

                if max_val > 8 {
                    col_widths[max_idx] -= 1;
                    current_width -= 1;
                    changed = true;
                }
            }
        }

        // Define box characters (Standard Light)
        let top_left = "┌";
        let top_right = "┐";
        let bottom_left = "└";
        let bottom_right = "┘";
        let horizontal = "─";
        let vertical = "│";
        let top_sep = "┬";
        let bottom_sep = "┴";
        let mid_left = "├";
        let mid_right = "┤";
        let mid_sep = "┼";

        // Helper to construct a separator line
        let make_sep = |left: &str, mid: &str, right: &str| -> String {
            let mut s = String::new();
            s.push_str("  "); // Indent
            s.push_str(left);
            for (i, width) in col_widths.iter().enumerate() {
                s.push_str(&horizontal.repeat(width + 2));
                if i < col_widths.len() - 1 {
                    s.push_str(mid);
                }
            }
            s.push_str(right);
            s
        };

        // Print Top Border
        println!("{}", make_sep(top_left, top_sep, top_right));

        // Print Headers
        print!("  {}", vertical);
        for (i, header) in self.headers.iter().enumerate() {
            let width = col_widths[i];
            let truncated = truncate(header, width);
            let visible_len = truncated.chars().count();
            let padding = width.saturating_sub(visible_len);
            print!(" {} {}{}", truncated.bold(), " ".repeat(padding), vertical);
        }
        println!();

        // Print Separator
        println!("{}", make_sep(mid_left, mid_sep, mid_right));

        // Print Rows
        for row in &self.rows {
            print!("  {}", vertical);
            for (i, cell) in row.iter().enumerate() {
                let clean_raw = sanitize_content(cell);
                let width = col_widths[i];
                let truncated_cow = console::truncate_str(&clean_raw, width, "...");
                let truncated = truncated_cow.to_string();

                let clean_trunc = strip_ansi(&truncated);
                let visible_len = clean_trunc.chars().count();
                let padding = width.saturating_sub(visible_len);

                print!(" {} {}{}", truncated, " ".repeat(padding), vertical);
            }
            println!();
        }

        // Print Bottom Border
        println!("{}", make_sep(bottom_left, bottom_sep, bottom_right));
    }
}

fn truncate(s: &str, max_width: usize) -> String {
    if s.chars().count() > max_width {
        let mut result: String = s.chars().take(max_width.saturating_sub(3)).collect();
        result.push_str("...");
        result
    } else {
        s.to_string()
    }
}

fn sanitize_content(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            _ => c,
        })
        .collect()
}

fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(&'[') = chars.peek() {
                chars.next();
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}
