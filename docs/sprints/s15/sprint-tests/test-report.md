# Sprint 15 Test Report

## Summary
- **Overall verdict:** PASS (all default + `--features python` + `--features native` + UI native + `wasm32-unknown-unknown` builds/clippy/tests green with 0 warnings; **6/6 GitHub Actions CI jobs `success`** on run [#45924350359](https://github.com/crussella0129/carbide/actions/runs/45924350359)).
- **Intent verified:** [INT-0004 — Dependency, toolchain, and CI modernization](../../../intents/INT-0004-dependency-and-toolchain-modernization.md).

## Acceptance Criteria Traceability
| Acceptance Criterion | Status | Evidence |
|----------------------|--------|----------|
| AC-1: Every intentional major dependency bump is verified across all CI jobs (lint, Ubuntu + Windows workspace tests, `native`, `python`, and `webui` native + wasm32) | PASS | Upgraded `hashbrown 0.17`, `glam 0.34`, `thiserror 2`, `pyo3 0.30`, `libloading 0.9`, `rfd 0.18` (superseding ADR-003 wasm `gtk3` workaround), and `egui`/`eframe 0.37` (upstream defaults); `zip` intentionally held at `2`; all local targets + `wasm32` clippy `-D warnings` + 6/6 CI jobs green on run [#45924350359](https://github.com/crussella0129/carbide/actions/runs/45924350359); [unit-tests.md](unit-tests.md), [e2e-tests.md](e2e-tests.md) |
| AC-2: `Cargo.toml` `rust-version` matches the actual minimum toolchain required by the dependency tree | PASS | Target-filtered `cargo metadata` verified exact MSRV match: `Cargo.toml` = `"1.88"` (`libloading 0.9.1`), `apps/carbide-ui/Cargo.toml` = `"1.95"` (`egui`/`eframe 0.37.0`); [integration-tests.md](integration-tests.md) |
| AC-3: `.github/dependabot.yml` covers `github-actions` plus both Cargo manifests targeting `dev` | PASS | `.github/dependabot.yml` verified with weekly `target-branch: "dev"` entries for `github-actions` (`/`), `cargo` (`/`), and `cargo` (`/apps/carbide-ui`); [integration-tests.md](integration-tests.md) |

## Hygiene & Compatibility Checks
- `apps/carbide-ui/src/app/*.rs`: max **819 lines** (`<= 1,000`), zero `#![allow(...)]`/`#[allow(...)]` attributes.
- `.crbd` `FORMAT_VERSION` unchanged at `2`; `state_io` + `format` + `carbide-store` persistence tests all passing unchanged.
- `Cargo.lock` deduplicated `hashbrown` to a single `0.17.1` line.

## Detailed Reports
- [Unit tests](unit-tests.md)
- [Integration tests](integration-tests.md)
- [End-to-end tests](e2e-tests.md)
