# PR: stabilize build/deps/lock behavior and unify .cx artifact paths

## Why This Is Safe
- Strict validation gates pass:
  - `cargo test --all-targets` -> PASS
  - `cargo clippy --all-targets --all-features -- -D warnings` -> PASS
- Added targeted unit coverage for:
  - object naming uniqueness
  - lock comparison strictness
  - test recompilation freshness logic
  - import source persistence
- Behavior smoke checks validated critical runtime paths and strict failure semantics.

## What Changed By Subsystem
- `build core/test/watch`
  - Canonical `.cx/<profile>/bin` artifact helpers and reuse.
  - Deterministic object file naming to prevent collisions.
  - `cx watch` rebuild-only behavior in non-test mode.
  - Test caching now accounts for global inputs (project objs/libs/modules/config).
- `deps fetch/manage`
  - Added `FetchOptions`, `FetchResult`, `fetch_dependencies_with_options`.
  - Lock enforcement configurable for update workflows.
  - Include/module dedupe and existence filtering.
  - Streamed prebuilt archive download (`io::copy`).
- `doctor/target commands`
  - Strict lock comparison (missing/extra/URL mismatch).
  - `sync` refuses stale lockfile.
  - `target add/remove/default` explicitly deferred with non-zero exit and `--profile` guidance.
- `package/ide/docker`
  - Unified artifact paths to `.cx`.
  - `cx package` default output now at project root.
  - Packaging now streams files to zip writer.
- `docs`
  - README updated for lock/sync strictness, target deferral, profile guidance, and watch semantics.

## Behavioral Deltas Users Notice
- `cx watch`: rebuilds only; no automatic program execution in non-test mode.
- `cx lock --check`: now fails on missing/extra/URL mismatch.
- `cx sync`: fails fast when lockfile is stale.
- `cx target add/remove/default`: deferred command surface with explicit guidance to `cx build --profile <name>`.
- `cx package`: default artifact `./<name>-v<version>.zip`; binary source `.cx/release/bin/...`.

## Validation Evidence
| Command | Result | Elapsed |
|---|---|---|
| `cargo test --all-targets` | PASS | 10.20s |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS | 3.38s |

## Smoke Check Evidence
| Check | Result | Notes |
|---|---|---|
| `cx watch` rebuild-only | PASS | rebuild marker present; `Running...` absent |
| `cx package` output/binary path | PASS | root zip exists; `.cx/release/bin/<name>.exe` exists |
| `cx lock --check` missing lock entry | PASS | non-zero exit + missing detected |
| `cx lock --check` extra lock entry | PASS | non-zero exit + extra detected |
| `cx lock --check` URL mismatch | PASS | non-zero exit + URL mismatch detected |
| `cx sync` stale lock refusal | PASS | non-zero exit + refusal message |
| `cx target add` deferred non-zero | PASS | deferred + profile hint |
| `cx target remove` deferred non-zero | PASS | deferred + profile hint |
| `cx target default` deferred non-zero | PASS | deferred + profile hint |

## PR Checklist
- [x] Scope constrained to stabilization files only (no unrelated tracked files).
- [x] Strict tests and clippy clean.
- [x] Docs aligned to runtime behavior.
- [x] Deferred target behavior explicitly communicated.
- [x] No version bump in this PR.

## Merge Criteria
- [ ] CI green on supported platforms.
- [ ] Maintainer review approval.
- [ ] Confirmation of deferred target policy acceptance.
