//! Docker configuration generator.
//!
//! This module provides the `cx docker` command which generates a multi-stage
//! Dockerfile for containerized C/C++ builds.
//!
//! ## Generated Files
//!
//! - `Dockerfile` - Multi-stage build (Ubuntu-based)
//! - `.dockerignore` - Excludes build artifacts

use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;

pub fn generate_docker_config() -> Result<()> {
    println!("{} Generating Docker Configuration...", "🐳".blue());

    if Path::new("Dockerfile").exists() {
        println!("{} Dockerfile already exists.", "!".yellow());
        return Ok(());
    }

    // Determine project name for the binary
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("app"))
        .to_string_lossy();

    // Multi-stage build
    let dockerfile_content = format!(
        r#"# Stage 1: Build
FROM ubuntu:latest AS builder

# Install dependencies (C++ compiler and Rust for caxe)
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    gcc \
    g++ \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Install Rust (to install caxe)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${{PATH}}"

# Install caxe
RUN cargo install caxe

# Build Project
WORKDIR /app
COPY . .
RUN cx build --release

# Stage 2: Runtime
FROM ubuntu:22.04-slim

# Copy artifacts
COPY --from=builder /app/build/bin/{} /usr/local/bin/app

# Run
CMD ["app"]
"#,
        project_name
    );

    fs::write("Dockerfile", dockerfile_content).context("Failed to write Dockerfile")?;

    // .dockerignore
    let ignore_content = "build/\n.git/\n.cx/\nvendor/\n";
    if !Path::new(".dockerignore").exists() {
        fs::write(".dockerignore", ignore_content)?;
    }

    println!("{} Created Dockerfile & .dockerignore", "✓".green());
    println!("   Run: docker build -t {} .", project_name);

    Ok(())
}
