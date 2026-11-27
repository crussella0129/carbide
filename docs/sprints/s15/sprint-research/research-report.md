# Sprint 15 Research Report — Dependency, Toolchain, and CI Modernization

## Intents Reviewed
- [INT-0004 — Dependency, toolchain, and CI modernization](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) (state: `proposed`)

## Summary
Audited both Cargo workspaces (`Cargo.toml` root engine workspace and `apps/carbide-ui/Cargo.toml` standalone UI workspace), `.github/workflows/ci.yml`, `.github/dependabot.yml`, and current `crates.io` releases against our active toolchain (`rustc 1.99.1`, `Python 3.13.11`, `wasm32-unknown-unknown`). Every candidate crate has a clean, verifiable upgrade path, `rfd 0.18` completely removes the ADR-003 wasm32 `gtk3` workaround, and the root workspace's `rust-version = "1.75"` is already stale relative to its own `Cargo.lock` (`delaunator 1.1.0` requires `1.87`).

## Findings

### 1. Root Workspace Dependencies (`Cargo.toml`, `crates/*`)
- **Stale MSRV:** `Cargo.toml:14` declares `rust-version = "1.75"`, but `cargo metadata` shows the current `Cargo.lock` already requires **1.87** (`delaunator 1.1.0`, `hashbrown 0.17.1` via `indexmap 2.14.0` requiring `1.85`).
- **`hashbrown` (`0.14` → `0.17`):** `Cargo.lock` currently duplicates `hashbrown 0.14.5` and `0.17.1` (pulled by `indexmap`). Bumping `[workspace.dependencies].hashbrown` to `"0.17"` (MSRV `1.85.0`) deduplicates the lockfile; all usages in `carbide-core`, `carbide-formula`, and `carbide-store` use standard `HashMap` APIs (`new`, `insert`, `get`, `iter`, `collect`, `serde`).
- **`glam` (`0.27` → `0.34`):** Used solely for `glam::Vec2` in `crates/carbide-tess` (`lib.rs`, `square.rs`, `hex.rs`, `triangle.rs`, `voronoi.rs`); `glam 0.34.3` requires MSRV `1.85.0`.
- **`thiserror` (`1` → `2`):** Standard `#[derive(Error)]` / `#[error(...)]` / `#[from]` usage across `carbide-core`, `carbide-tess`, `carbide-formula`, `carbide-store`; `thiserror 2.0.21` requires MSRV `1.77`.
- **`pyo3` (`0.23` → `0.30`):** Used in `crates/carbide-formula/src/python/mod.rs` behind `--features python`. `pyo3 0.30.1` (MSRV `1.85`) supports Python 3.13/3.14+ and renames `Python::with_gil` → `Python::attach` (2 call sites in `python/mod.rs:39, 57`). Python 3.13.11 is installed locally and in CI (`actions/setup-python` `3.13`), enabling both local and CI verification.
- **`libloading` (`0.8` → `0.9`):** Used in `crates/carbide-formula/src/transpile/native.rs` behind `--features native` (single `Library::new` + `get` call site). `libloading 0.9.1` requires MSRV `1.88.0`.
- **`zip` (`2` — intentionally held at `2`):** `carbide-store` uses `zip = { version = "2", default-features = false, features = ["deflate"] }` (MSRV `1.73`). Jumping 7 majors (`2` → `9`) introduces high churn on `.crbd` I/O with no functional or security benefit right now; keeping `zip 2` preserves low-risk `.crbd` stability and lets Dependabot track future updates on `dev`.

### 2. UI Workspace Dependencies (`apps/carbide-ui/Cargo.toml`, `apps/carbide-ui/src/`)
- **`rfd` (`0.14` → `0.18`) — Supersedes ADR-003:** Inspected `rfd-0.18.0/build.rs`: it explicitly matches `_ if is_wasm => {}` before checking native Linux backends. Thus `apps/carbide-ui/Cargo.toml:38` can drop the ADR-003 `features = ["gtk3"]` workaround and use `rfd = { version = "0.18", default-features = false }` on `wasm32`, and `rfd = "0.18"` on native (`xdg-portal` + `wayland` on Linux, requiring no `libgtk-3-dev` headers on Ubuntu CI). MSRV `1.92`.
- **`egui` / `eframe` (`0.29` → `0.37`):** Latest stable `0.37.0` (MSRV `1.95`, well within `rustc 1.99.1` and CI `stable`). All `egui`/`eframe` usage is cleanly encapsulated inside `apps/carbide-ui/src/` (`main.rs`, `ribbon.rs`, `format.rs`, `conditional.rs`, `app/*.rs`), with zero leakage into root crates. Expected mechanical API adaptations across 0.29 → 0.37: `ui.close_menu()` → `ui.close()`, `rect_stroke` (`StrokeKind`), tooltip/popup API (`show_tooltip_at_pointer`), menu bar (`egui::menu::bar` → `MenuBar`), and `eframe::App` method signature if updated in 0.37.
- **UI MSRV:** `apps/carbide-ui/Cargo.toml` currently omits `rust-version`. Setting `rust-version` explicitly from `cargo metadata --manifest-path apps/carbide-ui/Cargo.toml` after upgrades aligns both manifests with AC-2.

### 3. Dependabot & CI Configuration (`.github/dependabot.yml`, `.github/workflows/ci.yml`)
- `.github/dependabot.yml` currently only monitors `github-actions` (`/` → `dev`). Expanding it to add `package-ecosystem: "cargo"` entries for `directory: "/"` and `directory: "/apps/carbide-ui"` (both `schedule.interval: "weekly"`, `target-branch: "dev"`) satisfies AC-3.
- `.github/workflows/ci.yml` already runs all 6 required verification gates on `dev` pushes and PRs: `lint` (`rustfmt + clippy`), `test` (`ubuntu-latest`, `windows-latest`), `native` (`--features native`), `python` (`--features python`), and `webui` (native `fmt`/`clippy`/`test` + `wasm32-unknown-unknown` build).

## Gaps & Risks
- **`egui`/`eframe 0.37` breaking API surface:** Spanning 8 minor releases (0.29 → 0.37) may touch `ribbon.rs`, `app/mod.rs`, `app/dialogs.rs`, `app/render_*.rs`, and `main.rs` (`WebRunner` on wasm32). Mitigated by compiling both native (`--all-targets`) and `wasm32-unknown-unknown`, passing all 315 UI tests, and verifying on Ubuntu CI (`webui`).
- **`pyo3 0.30` / `libloading 0.9` feature-gated paths:** Default `cargo test --workspace` does not compile `--features python` or `--features native`. Mitigated by running `cargo clippy` and `cargo test` explicitly with `--features native` and `--features python` both locally and in CI.
- **Module hygiene invariant (`INT-0002`):** Keep every file under `apps/carbide-ui/src/app/` ≤ 1,000 lines and zero `#![allow(...)]`/`#[allow(...)]` attributes.

## Sources
1. `docs/intents/INT-0004-dependency-and-toolchain-modernization.md` — target intent and acceptance criteria.
2. `Cargo.toml`, `crates/*/Cargo.toml`, `apps/carbide-ui/Cargo.toml` — workspace manifests, feature flags, and current dependency pins.
3. `crates/carbide-formula/src/python/mod.rs`, `crates/carbide-formula/src/transpile/native.rs`, `apps/carbide-ui/src/main.rs` — feature-gated `pyo3`/`libloading` and `eframe` entrypoints.
4. `docs/history/decisions-legacy.md` (ADR-003) & `rfd-0.18.0/build.rs` — verification that wasm32 no longer requires the `gtk3` workaround.
5. `.github/dependabot.yml` & `.github/workflows/ci.yml` — Dependabot and 6-job CI matrix configuration.
