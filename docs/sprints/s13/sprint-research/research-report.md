# Sprint 13 Research Report

## Intents Reviewed
- [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) — selected; relevance: primary Sprint 13 scope — decompose the 7,429-line `apps/carbide-ui/src/app.rs` monolith into cohesive submodules under `apps/carbide-ui/src/app/` with zero behavioral change; current state: `proposed`.
- [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) — reviewed; relevance: downstream consumer that benefits from separated per-lattice renderers and commands; kept `proposed` for a subsequent sprint.

## 1. Sprint Goal
Decompose `apps/carbide-ui/src/app.rs` (7,429 lines, ~60% of `carbide-ui`) into cohesive, well-bounded Rust submodules under `apps/carbide-ui/src/app/` such that no file exceeds 1,000 lines, the public crate API (`carbide_ui::CarbideApp`) and all 308 UI tests remain unchanged and green across native and `wasm32-unknown-unknown`, and zero new `#[allow(...)]` attributes or behavioral changes are introduced.

## 2. Existing Code Survey
| File | Relevance | Notes |
|------|-----------|-------|
| `apps/carbide-ui/src/app.rs` | high | 7,429-line monolith: constants (35–127), internal types (128–430), `CarbideApp` struct (431–578), `new` demo seeding (579–898), autosave/IO (899–925, 5255–5514), cell accessors & formatting/nav (926–1193, 2282–2603), command/ribbon dispatch (1194–1828), dialogs (1829–2281), edit/clipboard/undo/fill/resize (2604–3271), 4 lattice renderers (`draw_grid` 3272–4016, `paint_hex`/`draw_hex_grid` 4017–4481, `draw_triangle_grid` 4482–4852, `draw_voronoi_grid` 4853–5254), `eframe::App::update` (5516–5894), pure & Voronoi helpers (5895–6600), `#[cfg(test)] mod tests` (6601–7429). |
| `apps/carbide-ui/src/lib.rs` | high | Declares `mod app;` and `pub use app::CarbideApp;`. Switching `src/app.rs` to `src/app/mod.rs` requires zero changes to `lib.rs` and preserves the exact public API. |
| `apps/carbide-ui/src/main.rs` | low | Consumes `carbide_ui::CarbideApp` on native and `wasm32`; unaffected by internal `app/` module layout. |
| `apps/carbide-ui/Cargo.toml` | low | Sub-workspace manifest; targets native and `wasm32-unknown-unknown` (`cfg(target_arch = "wasm32")` blocks live in `handle_save` / `handle_open` inside `io.rs`). |

## 3. External Sources
- None required (pure internal Rust module restructuring within a single crate).

## 4. Risks, Unknowns, Dependencies
- **Risk:** `cfg(target_arch = "wasm32")` code paths in `handle_save` / `handle_open` (`app/io.rs`) can fail to compile on `wasm32` even when native builds pass (e.g., missing imports inside cfg-gated blocks). Mitigation: verify `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` and GitHub Actions `webui` job.
- **Risk:** Moving functions across submodules could trigger unused-import or `dead_code` clippy warnings under `-D warnings` if visibility (`pub(super)`) or `use super::*` imports are sloppy. Mitigation: keep every extracted module's imports explicit and minimal, run `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` after every task, and ban any new `#[allow(...)]` attributes.
- **Risk:** Accidentally introducing subtle behavioral changes while moving 7,429 lines. Mitigation: extract verbatim line slices mechanically without rewriting logic, verify with `git diff` / line-count accounting, and run all 308 UI tests plus the full 331-test workspace suite.
- **Note (pre-existing observation, intentionally out of scope):** Lines 5293 and 5331 in `handle_save` / `handle_open` (`#[cfg(target_arch = "wasm32")]`) still pass `&["tscl"]` to `rfd::AsyncFileDialog::add_filter` while native passes `&["crbd"]` / `&["crbd", "tscl"]`. Preserved verbatim in Sprint 13 to honor `INT-0002`'s zero-behavioral-change contract; logged as follow-up technical debt.
- **Sprint-boundary intake note:** Dependabot opened PR #194 (`actions/setup-python` 5→7) and PR #195 (`actions/checkout` 4→7) targeting `dev`; kept unmerged during Sprint 13 per `mergePolicy: human-approve` and the hosted-updater intake rule.

## 5. Recommended Approach
Primary: Replace `apps/carbide-ui/src/app.rs` with `apps/carbide-ui/src/app/mod.rs` and 14 focused child submodules (`types.rs`, `helpers.rs`, `voronoi_helpers.rs`, `tests.rs`, `render_square.rs`, `render_hex.rs`, `render_triangle.rs`, `render_voronoi.rs`, `demos.rs`, `io.rs`, `dialogs.rs`, `cells.rs`, `commands.rs`, `edit.rs` — 15 files total) in 4 incremental, independently compiling and clippy-clean commits (`T-001`–`T-004`). Because every submodule is a child of `mod app`, Rust's module privacy lets `impl CarbideApp` blocks in child submodules access all private `CarbideApp` fields without widening visibility outside `mod app`, while cross-module inherent methods and sibling helper/type items use `pub(super)`.

Alternative considered: Split only the 4 lattice renderers and leave the remaining ~5,300 lines in `app/mod.rs`. Rejected because `INT-0002` AC-1 requires no file under `apps/carbide-ui/src/app/` to exceed ~1,000 lines.

Rationale: Every resulting file lands between ~230 and ~850 lines (well below 1,000), `src/lib.rs` and the public API stay untouched, and downstream `INT-0003` / `INT-0005` work gains small, isolated edit targets.

## Artifacts
- Full line map of `apps/carbide-ui/src/app.rs` (7,429 lines) verified via Python AST/heading scan on `dev` (`639946d`).
