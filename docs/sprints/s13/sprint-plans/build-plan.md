# Sprint 13 Build Plan

## Intents
- [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) — state: `planned`; acceptance criteria covered: AC-1 (no single file under `apps/carbide-ui/src/app/` exceeds 1,000 lines), AC-2 (public API `carbide_ui::CarbideApp` and all 308 UI unit tests unchanged and green on native + `wasm32-unknown-unknown`), AC-3 (`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with zero new `#[allow(...)]` suppressions).

## Schema Tree
- Sprint 13: Modularize `apps/carbide-ui/src/app.rs` monolith (zero behavioral change)
  - Module root + pure types, helpers, and tests
    - T-001: Convert `src/app.rs` → `src/app/mod.rs` and extract `types.rs`, `helpers.rs`, `voronoi_helpers.rs`, and `tests.rs`
  - Per-lattice rendering modules
    - T-002: Extract `render_square.rs`, `render_hex.rs`, `render_triangle.rs`, and `render_voronoi.rs`
  - App bootstrap, persistence I/O, and floating dialogs
    - T-003: Extract `demos.rs`, `io.rs`, and `dialogs.rs`
  - Cell accessors, navigation/formatting, commands, and editing
    - T-004: Extract `cells.rs`, `commands.rs`, and `edit.rs` so `mod.rs` and every submodule stay under 1,000 lines

## Visibility & Import Contract (Plan Critic C-001)
- **Zero visibility leakage outside `mod app`:** Only `pub struct CarbideApp` (in `mod.rs`) and `pub fn new` (in `demos.rs`) remain `pub`; `pub(crate) fn capture_state` and `pub(crate) fn restore_state` (in `io.rs`) preserve exact `pub(crate)` visibility.
- **Internal cross-module items use `pub(super)`:** Moved structs/enums, struct fields, and inherent methods in `types.rs`, helper functions/consts in `helpers.rs` and `voronoi_helpers.rs`, and cross-module `impl CarbideApp` methods in child submodules are marked `pub(super)`. Single-module-only helpers (e.g., `paint_hex`, `apply_square_border`) stay private.
- **Direct explicit imports (no unused glob imports in `mod.rs`):** Each child submodule imports only what it uses directly from `super::types::...`, `super::helpers::...`, `super::voronoi_helpers::...`, and `super::...`. `mod.rs` imports only what `mod.rs` itself references; `tests.rs` imports from `super::helpers::*`, `super::types::*`, and `super::voronoi_helpers::*` as needed. This eliminates `unused_imports` errors under `-D warnings` across both `--lib`/`wasm32` and `--tests` targets.

## Execution Sequence

### T-001: Convert `apps/carbide-ui/src/app.rs` → `apps/carbide-ui/src/app/mod.rs` and extract `types.rs`, `helpers.rs`, `voronoi_helpers.rs`, and `tests.rs`
- **Intent:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- **Touches:** `apps/carbide-ui/src/app.rs` (deleted/moved), `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/types.rs`, `apps/carbide-ui/src/app/helpers.rs`, `apps/carbide-ui/src/app/voronoi_helpers.rs`, `apps/carbide-ui/src/app/tests.rs`
- **Depends on:** (none)
- **Acceptance criterion:** AC-1 (extracted files ≤ 1,000 lines), AC-2 (all 308 UI tests pass with identical `CarbideApp` API), AC-3 (`fmt` + `clippy -D warnings` clean, zero new `#[allow(...)]`).
- **Success criterion (EARS):**
  - **WHEN** `apps/carbide-ui/src/app.rs` is converted into `apps/carbide-ui/src/app/{mod,types,helpers,voronoi_helpers,tests}.rs`, **THEN** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` on `apps/carbide-ui/Cargo.toml` **SHALL** exit 0 with all 308 unit tests passing and zero `#![allow(...)]`/`#[allow(...)]` attributes.
  - **WHEN** the line counts of `types.rs`, `helpers.rs`, `voronoi_helpers.rs`, and `tests.rs` are measured, **THEN** each file **SHALL** be ≤ 1,000 lines.
- **Notes:** Mark all cross-module items/fields/methods in `types.rs`, `helpers.rs`, and `voronoi_helpers.rs` as `pub(super)`. `tests.rs` holds the full `#[cfg(test)]` suite verbatim.

### T-002: Extract the four lattice renderers into `render_square.rs`, `render_hex.rs`, `render_triangle.rs`, and `render_voronoi.rs`
- **Intent:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- **Touches:** `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/render_square.rs`, `apps/carbide-ui/src/app/render_hex.rs`, `apps/carbide-ui/src/app/render_triangle.rs`, `apps/carbide-ui/src/app/render_voronoi.rs`
- **Depends on:** T-001
- **Acceptance criterion:** AC-1 (all four renderer modules ≤ 1,000 lines), AC-2 (all 308 tests pass, zero behavioral change), AC-3 (`fmt` + `clippy -D warnings` clean, zero new `#[allow(...)]`).
- **Success criterion (EARS):**
  - **WHEN** `draw_grid`, `paint_hex`/`draw_hex_grid`, `draw_triangle_grid`, and `draw_voronoi_grid` are moved into their respective `render_*.rs` submodules with `pub(super)` on the four `draw_*` entrypoints, **THEN** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` on `apps/carbide-ui/Cargo.toml` **SHALL** exit 0 with all 308 unit tests passing and zero `#[allow(...)]` attributes.
  - **WHEN** the line counts of `render_square.rs`, `render_hex.rs`, `render_triangle.rs`, and `render_voronoi.rs` are measured, **THEN** each file **SHALL** be ≤ 1,000 lines.
- **Notes:** Preserve every line of rendering and hit-test logic verbatim; keep `paint_hex` private to `render_hex.rs`.

### T-003: Extract demo seeding, persistence I/O, and floating dialogs into `demos.rs`, `io.rs`, and `dialogs.rs`
- **Intent:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- **Touches:** `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/demos.rs`, `apps/carbide-ui/src/app/io.rs`, `apps/carbide-ui/src/app/dialogs.rs`
- **Depends on:** T-002
- **Acceptance criterion:** AC-1 (all three modules ≤ 1,000 lines), AC-2 (native + wasm32 compile clean, `pub(crate)` preserved on `capture_state`/`restore_state`, all 308 tests pass), AC-3 (`fmt` + `clippy -D warnings` clean, zero new `#[allow(...)]`).
- **Success criterion (EARS):**
  - **WHEN** `pub fn new` moves to `demos.rs`, autosave/save/open/rebind/`pub(crate)` snapshot methods move to `io.rs`, and floating window + find/replace/note methods move to `dialogs.rs`, **THEN** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build --target wasm32-unknown-unknown` on `apps/carbide-ui/Cargo.toml` **SHALL** exit 0 with zero warnings, `capture_state`/`restore_state` remaining `pub(crate)`, and all 308 unit tests passing.
  - **WHEN** the line counts of `demos.rs`, `io.rs`, and `dialogs.rs` are measured, **THEN** each file **SHALL** be ≤ 1,000 lines.
- **Notes:** Preserve the `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` blocks inside `io.rs` verbatim (do not change `.tscl`/`.crbd` dialog filters in this pure refactor sprint).

### T-004: Extract cell accessors/navigation/formatting, commands/ribbon, and editing/clipboard/undo/resize into `cells.rs`, `commands.rs`, and `edit.rs`
- **Intent:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- **Touches:** `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/cells.rs`, `apps/carbide-ui/src/app/commands.rs`, `apps/carbide-ui/src/app/edit.rs`
- **Depends on:** T-003
- **Acceptance criterion:** AC-1 (every file under `apps/carbide-ui/src/app/` ≤ 1,000 lines, including `mod.rs`), AC-2 (`carbide_ui::CarbideApp` public API unchanged, all 308 UI tests + 331 workspace tests pass, native + wasm32 green), AC-3 (`fmt` + `clippy -D warnings` clean, zero new `#[allow(...)]`).
- **Success criterion (EARS):**
  - **WHEN** `cells.rs`, `commands.rs`, and `edit.rs` are extracted from `mod.rs`, **THEN** every `.rs` file under `apps/carbide-ui/src/app/` **SHALL** be ≤ 1,000 lines and `apps/carbide-ui/src/lib.rs` **SHALL** remain unchanged.
  - **WHEN** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build --target wasm32-unknown-unknown` run on `apps/carbide-ui/Cargo.toml` and `cargo test --workspace` runs on the root workspace, **THEN** all commands **SHALL** exit 0 with zero warnings, zero `#![allow(...)]`/`#[allow(...)]` attributes anywhere under `apps/carbide-ui/src/app/`, and 639/639 total tests passing.
- **Notes:** `mod.rs` retains module declarations, shared constants, `pub struct CarbideApp`, and `impl eframe::App for CarbideApp` (`update`), with no unused imports (Plan Critic C-001).
