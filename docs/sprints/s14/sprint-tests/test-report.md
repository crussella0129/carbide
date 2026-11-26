# Sprint 14 Test Report

## Summary
- **Overall verdict:** PASS (646/646 local tests passing, 0 warnings, `wasm32-unknown-unknown` green, 6/6 GitHub Actions CI jobs `success` on run [#45877964117](https://github.com/crussella0129/carbide/actions/runs/45877964117)).
- **Intent verified:** [INT-0003 — Lattice UI parity and Voronoi operational unification](../../../intents/INT-0003-lattice-ui-parity-and-unification.md).

## Acceptance Criteria Traceability
| Acceptance Criterion | Status | Evidence |
|----------------------|--------|----------|
| AC-1: Voronoi supports cell formatting (`voronoi_formats` persisted in `UiSnapshot`), notes (`voronoi_notes` persisted in `UiSnapshot`), and copy/cut/paste without breaking older `.crbd` v2 snapshots (`#[serde(default)]`) | PASS | `state_io::tests::snapshot_round_trips_through_ui_state`, `state_io::tests::legacy_v2_snapshot_without_voronoi_formats_or_notes_deserializes_with_defaults`, `clipboard::tests::voronoi_*`, `app::voronoi_helpers::tests::voronoi_paste_targets_*`; [unit-tests.md](unit-tests.md), [integration-tests.md](integration-tests.md) |
| AC-2: Multi-cell Voronoi marquee selections (`Selection.extra`) operate through format, clear, widget toggle, and copy pipelines rather than ignoring extras via `primary_cells()` | PASS | `active_selection_cells()` (`cells.rs`), `clear_active`/`copy_selection`/`cut_selection`/`paste` (`edit.rs`), `toggle_multi_lattice_widget` (`commands.rs`), `app::helpers::tests::toggle_widget_kind_*`; [unit-tests.md](unit-tests.md) |
| AC-3: Shared helpers/traits eliminate copy-pasted dispatch arms across square, hex, triangle, and Voronoi where semantics agree, covered by unit tests | PASS | Unified `FormatEdit { cell: CellId }`, `Action::Formats`, `merge_format_edits`, `format_range`/`toggle_range`, `active_cell_id`/`note_text`/`open_note`, `apply_multi_sheet_edits`/`cut_clear_targets`/`dedup_sheet_targets`, `clear_formula_refs`, and `toggle_widget_kind`; max file under `app/` is 819 lines (`<= 1000`), zero `allow` attributes, 6/6 CI jobs green; [unit-tests.md](unit-tests.md), [e2e-tests.md](e2e-tests.md) |

## Additional Verification
- **Wasm32 dialog filter parity (`apps/carbide-ui/src/app/io.rs`):** verified `handle_save` uses `&["crbd"]` and `handle_open` uses `&["crbd", "tscl"]` on both native and `wasm32-unknown-unknown`, and both local and CI `wasm32-unknown-unknown` builds succeeded with 0 warnings.

## Detailed Reports
- [Unit tests](unit-tests.md)
- [Integration tests](integration-tests.md)
- [End-to-end tests](e2e-tests.md)
