# Changelog

All notable changes to caxe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.9] - 2026-02-16
- Clarified framework support levels in `cx framework` with explicit statuses:
  - `daxe`: `integrated`
  - `fmt`, `spdlog`, `json`, `catch2`: `dependency-alias`
- `cx framework add <name>` now rejects dependency-alias entries with non-zero exit and exact `cx add <name>` guidance
- Improved framework info/list output with recommended commands per entry
- Kept backward compatibility for existing `[build].framework` alias values:
  - Build continues with warning
  - Explicit migration hint to `cx add <name>` is shown
- Replaced fragile framework config editing with section-aware `[build]` key mutation/removal logic
- Kept `cx target add/remove/default` visible while marking them deferred in v0.3.x help and runtime output
- Standardized deferred target mutation failures to non-zero with explicit profile-first guidance:
  - configure `[profile:<name>]`
  - run `cx build --profile <name>`
- Improved `cx target list` UX with a clear deferred-status banner and profile-based setup hint
- Added dedicated CI lint gate:
  - `cargo clippy --all-targets --all-features -- -D warnings`
- Aligned CI test execution to explicit target coverage:
  - `cargo test --all-targets --verbose`
- Added targeted regression coverage for framework metadata/mutation behavior and target deferred messaging/help surface
- Included dependency and workflow updates already present in `HEAD` (cargo deps + GitHub Actions deps bumps)

## [0.3.8] - Defects-First Stabilization & Governance 🛠️
- Canonicalized artifact paths to `.cx/<profile>/bin` across build/package/IDE/docker flows
- Updated `cx watch` non-test mode to rebuild-only behavior (no auto-run)
- Made `cx lock --check` strict (missing/extra/URL mismatch) and `cx sync` fail-fast on stale lockfiles
- Made `cx target add/remove/default` explicit deferred operations with non-zero exit and `--profile` guidance
- Added deterministic object naming and improved test recompilation freshness logic
- Persisted import source scanning for non-`src` project layouts
- Improved performance in cache pruning, dependency fetching, and package zip streaming
- Added dual licensing (`MIT OR Apache-2.0`) with `LICENSE-MIT` and `LICENSE-APACHE`
- Added contributor/community governance docs and templates (`CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue/PR templates)
- Expanded test coverage and module documentation
- Improved CLI maintainability with extracted command handlers and safer progress-style fallbacks

---


## [0.3.7] - Faster Builds with Prebuilt Cache ⚡
- Prebuilt binary cache for faster dependency builds
- SDL3 project template
- Fix: Script mode binary path handling

## [0.3.6] - Cross-Compilation Profiles & Enhanced Formatting 🎯
- Cross-compilation profile support with `--profile` flag
- Enhanced code formatting options

## [0.3.5] - Arduino & Cross-Platform Support 🤖
- Arduino CLI integration (`cx build --arduino`, `cx upload`)
- Cross-platform target management (`cx target`)
- Toolchain enhancements

## [0.3.4] - Toolchain Management 🔋
- Interactive toolchain installer (`cx toolchain install`)
- Renamed `cx build --profile` to `cx build --trace`

## [0.3.3] - Script Mode & Polish 📜
- Script mode for running single C/C++ files directly

## [0.3.2] - Polish & Registry Expansion ✨
- Expanded library registry

## [0.3.1] - Speed, Safety, and Polish 🚀
- Advanced optimizations (LTO, sanitizers)

## [0.3.0] - Parallel Builds & TDD 🚀
- Lock-free parallel compilation with rayon
- Test-driven development support
- Modern CLI with colors and Unicode

## [0.2.10] - Symmetric Box Styling 🎨
- Fix: Box styling symmetry issues

## [0.2.9] - Dry-Run Mode & Modern Styling 🔍
- Dry-run mode (`cx build --dry-run`)
- Modern CLI styling

## [0.2.8] - Verbose Mode & Philosophy 🔍
- Verbose mode (`-v`, `--verbose`)
- PHILOSOPHY.md

## [0.2.7] - Doctor Command & Toolchain Improvements 🩺
- `cx doctor` command
- Toolchain improvements

## [0.2.6] - Toolchain Discovery System 🔧
- Automatic toolchain discovery
- Interactive toolchain selection
- Enhanced `cx info`

## [0.2.5] - Graphics Ready (SDL2 & OpenGL) 🎨
- SDL2 and OpenGL support

## [0.2.4] - Documentation Made Easy 📚
- `cx doc` command (Doxygen)

## [0.2.3] - Scriptable Builds with Rhai 📜
- Rhai scripting support

## [0.2.2] - Windows Native Support 🖥️
- Native Windows/MSVC support

## [0.2.1] - Smart Header Tracking 🧠
- Header dependency tracking

## [0.2.0] - Parallel Builds, Rich Progress Bars & Linting 🚀
- Parallel build engine
- Rich progress interface
- Static analysis (`cx check`)

## [0.1.9] - Init & Cache Management 📦
- Project initialization (`cx init`)
- Cache management

## [0.1.8] - Search & Lockfiles 🔐
- Registry search
- Lockfile support (`cx.lock`)

## [0.1.7] - Remote Registry & Self-Upgrades 📡
- Remote registry
- Self-update (`cx upgrade`)

## [0.1.6] - Registry Aliases & Easy Installers 📦
- Registry aliases
- Automatic installers

## [0.1.5] - Stability & Smart Linking 🚀
- Smart linking

## [0.1.4] - Distribution, Formatting & System Packages 🚀
- `cx package` command
- `cx fmt` command
- System package support

## [0.1.3] - Scripts & C Support 📜
- Pre/post build scripts
- C language support

## [0.1.2] - Better Build Artifacts 🏗️
- Improved build artifacts

## [0.1.1] - Compiler Selection & System Info
- Compiler selection
- System info display

## [0.1.0] - Initial Release 🚀
- Zero-config C/C++ project creation
- Automatic toolchain detection
- Basic build and run commands

---

[Unreleased]: https://github.com/dhimasardinata/caxe/compare/v0.3.9...HEAD
[0.3.9]: https://github.com/dhimasardinata/caxe/compare/v0.3.8...v0.3.9
[0.3.8]: https://github.com/dhimasardinata/caxe/compare/v0.3.7...v0.3.8
[0.3.7]: https://github.com/dhimasardinata/caxe/compare/v0.3.6...v0.3.7
[0.3.6]: https://github.com/dhimasardinata/caxe/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/dhimasardinata/caxe/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/dhimasardinata/caxe/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/dhimasardinata/caxe/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/dhimasardinata/caxe/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/dhimasardinata/caxe/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/dhimasardinata/caxe/compare/v0.2.10...v0.3.0
[0.2.10]: https://github.com/dhimasardinata/caxe/compare/v0.2.9...v0.2.10
[0.2.9]: https://github.com/dhimasardinata/caxe/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/dhimasardinata/caxe/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/dhimasardinata/caxe/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/dhimasardinata/caxe/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/dhimasardinata/caxe/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/dhimasardinata/caxe/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/dhimasardinata/caxe/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/dhimasardinata/caxe/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/dhimasardinata/caxe/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/dhimasardinata/caxe/compare/v0.1.9...v0.2.0
[0.1.9]: https://github.com/dhimasardinata/caxe/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/dhimasardinata/caxe/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/dhimasardinata/caxe/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/dhimasardinata/caxe/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/dhimasardinata/caxe/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/dhimasardinata/caxe/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/dhimasardinata/caxe/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/dhimasardinata/caxe/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/dhimasardinata/caxe/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/dhimasardinata/caxe/releases/tag/v0.1.0
