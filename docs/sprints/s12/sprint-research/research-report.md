# Sprint 12 Research Report

## Intents Reviewed
- [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) — created & selected; relevance: primary Sprint 12 scope (restore green Rust 1.99 CI, finish ADR-015 repo URLs, refresh stale docs/CI branch trigger); current state: `proposed`.
- [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) — created; relevance: key structural improvement identified in assessment (`app.rs` is 7,433 lines); current state: `proposed`.
- [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) — created; relevance: ADR-013 deferred lattice UI unification and Voronoi operational parity; current state: `proposed`.
- [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) — created; relevance: 6-month dependency/toolchain modernization candidate (`egui 0.29`, `rfd 0.14`, `pyo3 0.23`, Dependabot cargo ecosystems); current state: `proposed`.
- [INT-0005](../../../intents/INT-0005-post-rename-product-roadmap.md) — created; relevance: deferred post-v161 product roadmap items awaiting user prioritization; current state: `proposed`.

## 1. Sprint Goal
Assess the entire Carbide repository after a 6-month hiatus, migrate its legacy Sprint Loops state onto the Book v2 substrate (contract 4), and execute a tight, zero-ambiguity re-entry restoration sprint (`INT-0001`) that fixes CI-blocking Rust 1.99 lint failures in `apps/carbide-ui`, finishes the already-approved ADR-015 repo URL updates, enables CI on pushes to `dev`, and clears stale Phase-0/Electron documentation and dead-code lint suppressions — while capturing deeper architectural and feature candidates (`INT-0002`–`INT-0005`) as `proposed` intents for user prioritization.

## 2. Existing Code Survey
| File | Relevance | Notes |
|------|-----------|-------|
| `apps/carbide-ui/src/app.rs` | high | 7,433-line UI monolith; 21 Rust 1.99 `float_literal_f32_fallback` clippy errors (`egui::Stroke::new`), 4 stale `#[allow(dead_code)]` attributes on live Voronoi helpers, preserved `…/tescellate` About URL, outdated About prose ("squares, hexagons"). |
| `Cargo.toml` | high | Workspace manifest (5 crates: `core`, `tess`, `formula`, `store`, `cli`); line 17 still points `repository` at `crussella0129/tescellate`; MSRV `1.75`. |
| `apps/carbide-ui/Cargo.toml` | medium | Separate workspace for egui/eframe 0.29 UI (native + wasm32); compiles clean on wasm32 except for the 21 `app.rs` warnings. |
| `.github/workflows/ci.yml` | high | 5 CI jobs (`lint`, `test` ubuntu/windows, `native`, `python`, `webui` native+wasm32); `on.push.branches` only lists `[main]`, missing new `work` branch `dev`. |
| `.github/dependabot.yml` | medium | Newly created by `deploy-substrate.sh` targeting `dev` for `github-actions`; `cargo` ecosystems deferred to `INT-0004`. |
| `CLAUDE.md` | high | Still describes deleted `apps/desktop/` (Electron + React + TS) and JSON-RPC IPC, plus old `git@github.com:crussella0129/tescellate.git` URL. |
| `README.md` | high | Still claims "Status: Phase 0 — foundation only. Nothing user-visible works yet", lists `Node.js 20+`, and has placeholder build steps. |
| `crates/carbide-cli/src/main.rs` | medium | 17-line usage stub; line 1 doc comment has redundant ``(becomes `carbide-cli` in v161)`` artifact from rename sweep. |
| `crates/carbide-formula/src/engine.rs` | medium | 2,433 lines (`WorkbookEngine` + 1,353 lines of tests); all 174 unit + 53 integration/fuzz tests pass; `native` clippy clean. |
| `crates/carbide-tess/src/lib.rs` | medium | 657 lines; Square, Hex (pointy/flat), Triangle, Voronoi implemented (87 tests pass); `LatticeKind::Parallelogram` still returns `None`. |
| `crates/carbide-core/src/lib.rs` | low | 138 lines; clean data model + `Sheet.lattice_config` (`FORMAT_VERSION = 2`); 8 tests pass. |
| `crates/carbide-store/src/lib.rs` | low | 519 lines; `.crbd` v2 zip store with v0/v1 back-compat and opaque `ui.json`; 9 tests pass. |
| `apps/carbide-ui/src/state_io.rs` | low | 290 lines; `UiSnapshot` + wasm localStorage autosave; missing Voronoi formats/notes (`INT-0003`). |
| `docs/history/decisions-legacy.md` | medium | Migrated `decisions.md` (ADR-001..ADR-015); ADR-015 documents the exact 3 preserved URLs to flip once GitHub repo rename completes. |

## 3. External Sources
- [rust-lang/rust#154024](https://github.com/rust-lang/rust/issues/154024) — future-incompatibility lint `float_literal_f32_fallback` emitted by Rust 1.99.1 on unsuffixed float literals passed to `impl Into<f32>` (`egui::Stroke::new`).

## 4. Risks, Unknowns, Dependencies
- **Risk:** `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` currently exits 101 on Rust 1.99.1 (21 errors in `app.rs`), blocking the `webui` CI job on every PR until fixed.
- **Risk:** Editing `apps/carbide-ui/src/app.rs` manually across 21 call sites could miss one site or introduce formatting drift; mitigation is verifying with both `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo build --target wasm32-unknown-unknown`.
- **Risk:** Local machine does not necessarily have CPython 3.13 linked for `cargo test -p carbide-formula --features python`; mitigation is verifying the `python` CI job on GitHub Actions once `dev` is pushed (or checking locally if Python is configured).
- **Unknown:** User preference among the deeper improvement tracks (`INT-0002` `app.rs` split, `INT-0003` Voronoi/lattice UI parity, `INT-0004` dependency upgrades, pruning 72 legacy remote branches, or jumping to `INT-0005` features). Mitigation: keep Sprint 12 strictly to zero-ambiguity `INT-0001` and surface `INT-0002`–`INT-0005` at the human-approve checkpoint.
- **Dependency:** `gh` CLI is authenticated for `crussella0129` (`repo`, `workflow` scopes) and GitHub repo is live at `https://github.com/crussella0129/carbide`.

## 5. Recommended Approach
Primary: Execute `INT-0001` in 4 small, atomic, zero-behavior-change tasks on `dev`:
1. `T-001`: Fix all 21 `float_literal_f32_fallback` literals (`_f32` suffix) and remove the 4 stale `#[allow(dead_code)]` attributes in `apps/carbide-ui/src/app.rs`.
2. `T-002`: Complete ADR-015 step 3 by flipping the 3 preserved `crussella0129/tescellate` URLs (`Cargo.toml`, `CLAUDE.md`, `apps/carbide-ui/src/app.rs`) and updating the About dialog lattice description + `crates/carbide-cli/src/main.rs` stale comment.
3. `T-003`: Update `.github/workflows/ci.yml` to run on `push` to `dev` as well as `main`.
4. `T-004`: Refresh `README.md` and `CLAUDE.md` to accurately document the current all-Rust workspace + `apps/carbide-ui` stack, real build/test/WASM commands, and Book v2 docs layout.

Alternative considered: Pause after substrate migration without fixing CI or opening a PR. Rejected because the user explicitly invoked `/sprint-loops continue` with `gh` remote steps, and any `dev -> main` checkpoint PR would fail the `webui` CI job without the Rust 1.99 lint fix.

Rationale: Delivers an immediately green, clean, Book-v2-backed `dev -> main` checkpoint PR while preserving user decision authority over all larger structural or feature investments (`INT-0002`–`INT-0005`, legacy remote branch cleanup).

## Artifacts
- Local baseline audit (2026-11-26, Rust 1.99.1):
  - Workspace: `cargo fmt` PASS, `cargo clippy --workspace --all-targets -- -D warnings` PASS, `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings` PASS, `cargo test --workspace` PASS (331/331).
  - UI (`apps/carbide-ui`): `cargo fmt` PASS, `cargo clippy --all-targets -- -D warnings` **FAIL** (21 `float_literal_f32_fallback` errors in `src/app.rs`), `cargo test` PASS (308/308), `cargo check --target wasm32-unknown-unknown` PASS with the same 21 warnings.
- Remote audit (`gh`): `https://github.com/crussella0129/carbide`, default branch `main`, 0 open PRs, 0 open issues, 72 legacy remote branches on `origin`.
