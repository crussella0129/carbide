Finalized - DO NOT EDIT

# Sprint 12 Build Plan

## Intents
- [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) — state: `planned`; acceptance criteria covered: AC-1 (workspace fmt/clippy/test + native clippy green), AC-2 (UI fmt/clippy/test + wasm32 build green with zero warnings), AC-3 (zero `#[allow(dead_code)]` in `app.rs`), AC-4 (all 3 ADR-015 repo URLs point to `crussella0129/carbide`), AC-5 (CI triggers `push` on `main` and `dev`), AC-6 (stale Phase-0/Electron prose updated with zero runtime/persistence regression).

## Schema Tree
- Sprint 12: Re-entry CI restoration, ADR-015 URL completion, and doc/lint hygiene
  - UI Rust 1.99 lint & dead-code cleanup
    - T-001: Fix 21 `float_literal_f32_fallback` literals and remove 4 stale `#[allow(dead_code)]` attributes in `apps/carbide-ui/src/app.rs`
  - Post-rename URL & CLI/About metadata completion
    - T-002: Update the 3 preserved `crussella0129/tescellate` URLs (`Cargo.toml`, `CLAUDE.md`, `apps/carbide-ui/src/app.rs`), About dialog lattice summary, and `crates/carbide-cli/src/main.rs` stale comment
  - CI branch alignment
    - T-003: Add `dev` to `on.push.branches` in `.github/workflows/ci.yml`
  - Developer & user documentation refresh
    - T-004: Refresh `README.md` and `CLAUDE.md` to reflect the current pure-Rust workspace + `apps/carbide-ui` stack and Book v2 layout

## Execution Sequence

### T-001: Fix 21 Rust 1.99 `float_literal_f32_fallback` clippy errors and remove 4 stale `#[allow(dead_code)]` attributes in `apps/carbide-ui/src/app.rs`
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- **Touches:** `apps/carbide-ui/src/app.rs`
- **Depends on:** (none)
- **Acceptance criterion:** AC-2 (UI fmt, clippy `-D warnings`, tests, and wasm32 build green with zero warnings) and AC-3 (zero `#[allow(dead_code)]` attributes in `apps/carbide-ui/src/app.rs`).
- **Success criterion (EARS):**
  - **WHEN** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build --target wasm32-unknown-unknown` run against `apps/carbide-ui/Cargo.toml` on Rust 1.99+, **THEN** all four commands **SHALL** exit 0 with zero `float_literal_f32_fallback`, `dead_code`, or other compiler/clippy warnings and all 308 unit tests passing.
  - **WHEN** `apps/carbide-ui/src/app.rs` is inspected for `#[allow(dead_code)]`, **THEN** zero occurrences **SHALL** remain.
- **Notes:** Add explicit `_f32` suffixes to the 21 unsuffixed float literals passed to `egui::Stroke::new(...)` (matching rustc's exact machine-applicable suggestions for rust-lang/rust#154024), delete the 4 obsolete `#[allow(dead_code)]` attributes on `SEED_FADE_OUTER`, `SEED_FADE_MAX_ALPHA`, `WIDGET_POP_OUT_OFFSET`, and `pop_out_widget_center` (all live in `draw_voronoi_grid`), and remove the stale `// Wired into the widget pass by T-004.` comment at line 6376.

### T-002: Complete ADR-015 post-rename repo URLs, About dialog text, and `carbide-cli` doc comment
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- **Touches:** `Cargo.toml`, `CLAUDE.md`, `apps/carbide-ui/src/app.rs`, `crates/carbide-cli/src/main.rs`
- **Depends on:** T-001
- **Acceptance criterion:** AC-1 (workspace fmt/clippy/native-clippy/tests green), AC-4 (all three ADR-015 preserved URLs point to `crussella0129/carbide`), and AC-6 (About dialog and `carbide-cli` stub comment reflect current post-rename reality with zero regression).
- **Success criterion (EARS):**
  - **WHEN** `git grep -i "crussella0129/tescellate"` runs across `Cargo.toml`, `CLAUDE.md`, and `apps/carbide-ui/src/app.rs`, **THEN** zero matches **SHALL** be found and all three sites **SHALL** reference `crussella0129/carbide`.
  - **WHEN** the About dialog description in `apps/carbide-ui/src/app.rs` and the module doc in `crates/carbide-cli/src/main.rs` are read, **THEN** they **SHALL** list all four implemented lattices (squares, hexagons, triangles, Voronoi) and contain no redundant `(becomes carbide-cli in v161)` artifact.
  - **WHEN** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings`, and `cargo test --workspace` run after updating `Cargo.toml` and `crates/carbide-cli/src/main.rs`, **THEN** all four commands **SHALL** exit 0 with zero warnings and all 331 workspace tests passing.
- **Notes:** Preserve `.gitignore`'s dual `.tescellate-cache/` + `*.tscl` rules and `docs/history/decisions-legacy.md` / `docs/work/completed-tasks.md` historical provenance untouched per ADR-015.

### T-003: Trigger GitHub Actions CI on pushes to the Book v2 `work` branch (`dev`)
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- **Touches:** `.github/workflows/ci.yml`
- **Depends on:** (none)
- **Acceptance criterion:** AC-5 (`.github/workflows/ci.yml` triggers `push` on both `main` and `dev`).
- **Success criterion (EARS):**
  - **WHEN** `.github/workflows/ci.yml` is inspected under `on.push.branches`, **THEN** the branch list **SHALL** include both `main` and `dev`, preserving all five existing CI jobs (`lint`, `test`, `native`, `python`, `webui`) intact.
- **Notes:** Keep YAML formatting clean and minimal; do not alter job definitions or toolchain versions (dependency/CI upgrades are scoped to `INT-0004`).

### T-004: Refresh stale Phase-0 / Electron documentation in `README.md` and `CLAUDE.md`
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- **Touches:** `README.md`, `CLAUDE.md`
- **Depends on:** T-002
- **Acceptance criterion:** AC-6 (`README.md` and `CLAUDE.md` accurately describe the current pure-Rust workspace + `apps/carbide-ui` stack and real build/test commands).
- **Success criterion (EARS):**
  - **WHEN** `README.md` and `CLAUDE.md` are checked for stale pre-v161 claims (`Status: Phase 0`, `Nothing user-visible works yet`, `apps/desktop`, `Node.js 20`, `JSON-RPC IPC`, `Tauri`, `prettier`, `eslint`, `npm install`, `npm run dev`), **THEN** zero occurrences **SHALL** remain (deferring pre-v161 Electron/IPC history to `docs/history/decisions-legacy.md`) and every documented `cargo` command **SHALL** match the working workspace and `apps/carbide-ui` manifests.
  - **WHEN** `CLAUDE.md` references architecture, Cargo features, and sprint state, **THEN** it **SHALL** document the `native` Cargo feature alongside `python`/`rhai`/`rustnative`, point to `docs/` (Project Book v2) and `docs/history/decisions-legacy.md`, and retain the `crussella0129/carbide` repository URL.
- **Notes:** Fix the unclosed `**` bold marker on `README.md:11`. Preserve `CLAUDE.md`'s shell-command guidance (`--manifest-path apps/carbide-ui/Cargo.toml`, no redundant `cd`, `gh run watch`) and core working norms.
