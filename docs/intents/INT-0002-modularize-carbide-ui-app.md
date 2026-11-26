# INT-0002 — Modularize `apps/carbide-ui/src/app.rs` monolith

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0002
- **State:** proposed
- **Work evidence:** none
- **Completion evidence:** none
- **Code evidence:** none
- **Test evidence:** none
- **Documentation evidence:** none

## Intent
Decompose `apps/carbide-ui/src/app.rs` (currently 7,433 lines) into cohesive submodules under `apps/carbide-ui/src/app/` (e.g., `mod.rs`, `demos.rs`, `commands.rs`, `dialogs.rs`, `render_square.rs`, `render_hex.rs`, `render_triangle.rs`, `render_voronoi.rs`, `voronoi_helpers.rs`) with zero behavioral changes.

## Acceptance criteria
1. No single file under `apps/carbide-ui/src/app/` exceeds ~1,000 lines (excluding tests if kept in a dedicated test submodule).
2. Public crate API (`carbide_ui::CarbideApp`) and all 308+ UI unit tests remain unchanged and green across native and `wasm32-unknown-unknown`.
3. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with zero new `#[allow(...)]` suppressions.

## Rationale
`app.rs` holds ~60% of the entire UI crate in one file (state, 4 demo seeds, 4 lattice renderers, 5 dialogs, keyboard/ribbon command dispatch, undo/redo, and 800+ lines of tests). Splitting it before adding CSV/`.xlsx` import or further lattice parity lowers merge conflict risk, review friction, and agent context cost.

## Alternatives
- **Keep growing `app.rs` inline:** simpler per-PR, but every UI change touches a 7.4k-line file and accelerates drift between the four lattice renderers.
- **Combine decomposition with a full trait rewrite (`INT-0003`):** higher risk; pure file decomposition first makes the subsequent trait unification easier to review and verify.

## Consequences
- One dedicated refactor sprint with no user-visible behavior change.
- Future UI sprints (`INT-0003`, `INT-0005`) get smaller, isolated diffs.

## Transition history
- 2026-11-26: created as `proposed` during Sprint 12 6-month project assessment.
