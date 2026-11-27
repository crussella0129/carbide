Finalized - DO NOT EDIT

# Sprint 15 Test Plan

## Intent Traceability
| Intent | Acceptance criterion | Build task / EARS clause | Verification |
|--------|----------------------|--------------------------|--------------|
| [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) | AC-1: Every intentional major dependency bump is verified across all CI jobs (lint, Ubuntu + Windows workspace tests, `native`, `python`, and `webui` native + wasm32) | T-001, T-002, T-003 / WHEN upgraded deps (`hashbrown 0.17`, `glam 0.34`, `thiserror 2`, `pyo3 0.30`, `libloading 0.9`, `rfd 0.18`, `egui`/`eframe 0.37`) compile and test locally and in CI (native + wasm32 `-D warnings`) THEN all 6 CI jobs and local targets exit 0 with 0 warnings | `verify_root_workspace_and_features`, `verify_ui_native_wasm32_and_hygiene`, `verify_github_actions_ci_on_dev` |
| [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) | AC-2: `Cargo.toml` `rust-version` matches the actual minimum toolchain required by the dependency tree | T-004 / WHEN target-filtered `cargo metadata` computes the normalized max dependency `rust_version` (with `edition == "2024"` → `(1, 85, 0)` floor) across supported targets THEN both manifests declare canonical `rust-version` equal to their exact resolved maximum | `verify_msrv_alignment` |
| [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) | AC-3: `.github/dependabot.yml` covers `github-actions` plus both Cargo manifests targeting `dev` | T-004 / WHEN `.github/dependabot.yml` is inspected THEN it defines weekly `dev`-targeted entries for `github-actions` (`/`), `cargo` (`/`), and `cargo` (`/apps/carbide-ui`) | `verify_dependabot_config` |

## Execution Order Guarantee
All verification gates below run in full after `T-004` completes (in addition to incremental verification after each task `T-001`–`T-004`).

## Unit Tests
### Root workspace + feature-gated engine suites (T-001, T-002)
- **Intent:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- `verify_root_workspace_and_features`:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace` (all 331+ workspace tests green, including `carbide-store` `.crbd` v2 persistence and `carbide-tess` geometry with `glam 0.34`)
  - `cargo clippy -p carbide-formula --features python --all-targets -- -D warnings` & `cargo test -p carbide-formula --features python` (all Python engine tests green on `pyo3 0.30` + Python 3.13)
  - `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings` & `cargo test -p carbide-formula --features native` (all native-compile pipeline tests green on `libloading 0.9`)
  - Confirm `Cargo.lock` deduplicates `hashbrown` to a single `0.17.x` line.

### UI crate native + wasm32 + hygiene (T-003)
- **Intent:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- `verify_ui_native_wasm32_and_hygiene`:
  - `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`
  - `cargo test --manifest-path apps/carbide-ui/Cargo.toml` (all 315+ UI tests green, including `state_io` and `format` serde compatibility on `egui 0.37`)
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown --all-targets -- -D warnings` (C-002: enforces `-D warnings` on `#[cfg(target_arch = "wasm32")]` codepaths in `main.rs`, `app/io.rs`, `state_io.rs`)
  - `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` (0 warnings; confirm `apps/carbide-ui/Cargo.toml` no longer enables `gtk3` on `wasm32` and ADR-003 comments/status are updated per C-004)
  - Verify `max(lines) <= 1000` for every `*.rs` under `apps/carbide-ui/src/app/` and zero `#!?[allow(` attributes under `apps/carbide-ui/src/app/`.

## Integration Tests
### MSRV & Dependabot configuration verification (T-004)
- **Intents:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- `verify_msrv_alignment`: run `cargo metadata --locked --format-version 1 --all-features --filter-platform <triple>` across supported targets (`x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` for root; plus `wasm32-unknown-unknown` for `apps/carbide-ui`), parse each dependency's `rust_version` as normalized `(major, minor, patch)` integer tuples (applying `(1, 85, 0)` floor for `edition == "2024"` crates without explicit `rust_version`, C-003), format canonically as `X.Y` (or `X.Y.Z` if patch > 0), and assert exact match with `Cargo.toml` `[workspace.package].rust-version` and `apps/carbide-ui/Cargo.toml` `[package].rust-version`.
- `verify_dependabot_config`: verify `.github/dependabot.yml` parses cleanly with `version: 2` and weekly `target-branch: "dev"` entries for `github-actions` (`/`), `cargo` (`/`), and `cargo` (`/apps/carbide-ui`).

## End-to-End Tests
- **Status:** possible
- `verify_github_actions_ci_on_dev`: push `dev` and confirm GitHub Actions CI (`rustfmt + clippy`, `build + test (ubuntu-latest)`, `build + test (windows-latest)`, `native compile pipeline`, `python engine`, `web front-end (wasm + native)`) concludes 6/6 `success` via `gh run watch`, plus `check-substrate.sh` (`substrate-complete`) and `check-book.sh` (`valid v2 Book`).
