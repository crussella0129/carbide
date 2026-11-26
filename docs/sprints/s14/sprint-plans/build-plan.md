# Sprint 14 Build Plan

## Intents
- [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) — state: `planned`; acceptance criteria covered: AC-1 (Voronoi formatting, notes, and copy/cut/paste with backward-compatible `.crbd` v2 `UiSnapshot` persistence), AC-2 (Voronoi multi-cell marquee `Selection.extra` wired through format, clear, widget toggle, and copy pipelines), AC-3 (shared helpers/types eliminate duplicated four-lattice dispatch and format-history boilerplate, covered by unit tests). Also includes the wasm32 `.crbd`/`.tscl` file-dialog filter parity fix in `apps/carbide-ui/src/app/io.rs`.

## Schema Tree
- Sprint 14: Lattice UI parity and Voronoi operational unification (`INT-0003`)
  - Unified formatting + history + Voronoi formatting & persistence
    - T-001: Unify `FormatEdit` / `Action::Formats` across all 4 lattices and wire Voronoi formatting (ribbon, `LayoutJob` renderer, marquee `Selection.extra`, `UiSnapshot`)
  - Voronoi notes + multi-lattice `OpenNote` + context menu & persistence
    - T-002: Add `voronoi_notes`, unify `OpenNote` / note helpers across all 4 lattices, add Voronoi note indicator/tooltip/context menu, and persist `voronoi_notes` in `UiSnapshot`
  - Voronoi clipboard + atomic cross-sheet cut/paste + marquee-aware clear / edit-begin
    - T-003: Unify `Clipboard` cut state via `Option<CutOrigin>` + `SourceLattice::Voronoi`, add atomic multi-sheet cut/paste (`apply_multi_sheet_edits` + `cut_clear_targets`), wire Voronoi copy/cut/paste + Escape + `clear_formula_refs`, and fix Voronoi `clear_active` / `begin_edit`
  - Multi-lattice widget toggle parity + wasm32 filter parity + unit tests
    - T-004: Extract shared `toggle_widget_kind` (discriminant-safe) across all 4 lattices (Toggle + Button, marquee-aware on Voronoi), fix wasm32 `.crbd`/`.tscl` dialog filters in `io.rs`, and add unit test coverage

## Architecture & Invariants (Incorporating Plan Critic C-001..C-005)
- **`.crbd` v2 compatibility (ADR-001, C-004):** `FORMAT_VERSION` remains `2`. New `UiSnapshot` fields (`voronoi_formats`, `voronoi_notes`) carry `#[serde(default)]` so pre-Sprint-14 `.crbd` snapshots deserialize cleanly with empty maps. Compatibility unit tests decode legacy v2 JSON directly via `serde_json::from_value::<UiSnapshot>(...).unwrap()` (not masked by `ui_state_to_snapshot`'s `.unwrap_or_default()`) and assert non-default legacy fields survive alongside empty Voronoi maps.
- **Module size & lint hygiene (INT-0002 / Rust 1.99):** Every `.rs` file under `apps/carbide-ui/src/app/` stays ≤ 1,000 lines, zero `#![allow(...)]`/`#[allow(...)]` attributes, and every `egui::Stroke::new(...)` uses explicit `_f32` literals.
- **Voronoi text & note rendering (C-003):** Centroid text uses `egui::text::LayoutJob` + `painter.layout_job` + `painter.galley` (centered on centroid, supporting `italic`, faux-`bold`, `font_size`, `text_color`, `negative_red`, and `draw_text_decorations`). Note marker dots paint outside the widget/empty-text early-`continue` guards so empty or widget cells with notes still display their marker.
- **Atomic cut/paste & single `CutOrigin` state (C-001, C-002):** `Clipboard` stores cut state as a single `Option<CutOrigin>` (`Grid(i32, i32)` | `Voronoi(Vec<VoronoiCoord>)`) with `has_cut()`, `cut_voronoi_cells()`, and `consume_cut()`. `Command::ClearMarquee` checks `self.clipboard.has_cut()`. Multi-sheet cut/paste executes atomically in a single `Action::Cells` undo step via `apply_multi_sheet_edits` and `cut_clear_targets`.
- **Widget scope (ADR-006, C-005):** Toggle and Button widgets are enabled across Square, Hex, Triangle, and Voronoi via `toggle_widget_kind` (matching `std::mem::discriminant`); Slider and ProgressBar remain Square-only.

## Execution Sequence

### T-001: Unify four-lattice formatting/history and wire Voronoi formatting + persistence
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- **Touches:** `apps/carbide-ui/src/app/types.rs`, `apps/carbide-ui/src/app/helpers.rs`, `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/demos.rs`, `apps/carbide-ui/src/app/cells.rs`, `apps/carbide-ui/src/app/commands.rs`, `apps/carbide-ui/src/app/edit.rs`, `apps/carbide-ui/src/app/render_voronoi.rs`, `apps/carbide-ui/src/app/io.rs`, `apps/carbide-ui/src/state_io.rs`, `apps/carbide-ui/src/app/tests.rs`
- **Depends on:** (none)
- **Acceptance criterion:** AC-1 (Voronoi formatting + `voronoi_formats` persisted in `UiSnapshot` with `#[serde(default)]`), AC-2 (Voronoi marquee `Selection.extra` cells formatted via `selection.cells()`), AC-3 (single `FormatEdit { cell: CellId }`, `Action::Formats`, `merge_format_edits`, and unified `format_range`/`toggle_range` across all 4 lattices).
- **Success criterion (EARS):**
  - **WHEN** `format_range` or `toggle_range` runs on Square, Hex, Triangle, or Voronoi (including a Voronoi marquee selection with `Selection.extra`), **THEN** every selected cell's format **SHALL** update in one undoable `Action::Formats` step with drag coalescing, and `undo`/`redo` **SHALL** restore/reapply all touched cells.
  - **WHEN** a Voronoi cell carries a `CellFormat`, **THEN** `draw_voronoi_grid` **SHALL** render its fill override (`fmt.fill`), formatted number text (`voronoi_cell_text`), text color (including `negative_red`), font size, `italic` via `LayoutJob`, faux-`bold`, and underline/strikethrough decorations (C-003), and the ribbon **SHALL** reflect `self.voronoi.formats.get(cursor)`.
  - **WHEN** a `UiSnapshot` with `voronoi_formats` round-trips through `snapshot_to_ui_state`/`ui_state_to_snapshot` AND a legacy v2 JSON without `voronoi_formats` is decoded via `serde_json::from_value::<UiSnapshot>(...).unwrap()`, **THEN** populated `voronoi_formats` **SHALL** round-trip losslessly and legacy JSON **SHALL** deserialize with an empty `FormatMap<VoronoiCoord>` while preserving non-default legacy fields (C-004).
- **Notes:** Fix the Voronoi status-bar selection hint in `mod.rs` to guard on `let n = self.voronoi.selection.cells().len(); if n > 1` (C-005). Update existing `merge_format_edits` tests in `tests.rs` to test `CellId` across all 4 lattices.

### T-002: Add Voronoi notes, multi-lattice `OpenNote`, hover tooltips, context menu, and persistence
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- **Touches:** `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/demos.rs`, `apps/carbide-ui/src/app/cells.rs`, `apps/carbide-ui/src/app/dialogs.rs`, `apps/carbide-ui/src/app/commands.rs`, `apps/carbide-ui/src/app/render_square.rs`, `apps/carbide-ui/src/app/render_hex.rs`, `apps/carbide-ui/src/app/render_voronoi.rs`, `apps/carbide-ui/src/app/io.rs`, `apps/carbide-ui/src/state_io.rs`
- **Depends on:** T-001
- **Acceptance criterion:** AC-1 (Voronoi notes + `voronoi_notes` persisted in `UiSnapshot` with `#[serde(default)]`), AC-3 (shared `active_cell_id`, `note_text`, `open_note` replace duplicated per-lattice note-open code and fix `RibbonAction::OpenNote` on non-square sheets).
- **Success criterion (EARS):**
  - **WHEN** `RibbonAction::OpenNote` or a cell context menu's "Edit note…" triggers on any active sheet (Square, Hex, Triangle, or Voronoi), **THEN** `note_cell` **SHALL** target the active sheet's cursor cell and `set_note` **SHALL** persist or clear the note in that sheet's `NoteMap`.
  - **WHEN** a Voronoi cell has a non-empty note (even if the cell is empty or hosts a widget, C-003) or an error value, **THEN** `draw_voronoi_grid` **SHALL** paint a note marker dot near the cell centroid outside the text/widget early-continue guards, display a hover tooltip (error detail or note text), and provide a right-click context menu (Copy, Cut, Paste, Paste values, Clear, Edit note…).
  - **WHEN** a `UiSnapshot` with `voronoi_notes` round-trips through `snapshot_to_ui_state`/`ui_state_to_snapshot` AND a legacy v2 JSON without `voronoi_notes` is decoded via `serde_json::from_value::<UiSnapshot>(...).unwrap()`, **THEN** populated `voronoi_notes` **SHALL** round-trip losslessly and legacy JSON **SHALL** deserialize with an empty map while preserving non-default legacy fields (C-004).
- **Notes:** Register `vec_pair_for!(voronoi_notes, carbide_tess::voronoi::VoronoiCoord)` in `state_io.rs`. Keep `render_voronoi.rs` well under 1,000 lines.

### T-003: Add Voronoi copy/cut/paste, atomic cross-sheet cut/paste, and marquee-aware `clear_active` / `begin_edit`
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- **Touches:** `apps/carbide-ui/src/clipboard.rs`, `apps/carbide-ui/src/app/edit.rs`, `apps/carbide-ui/src/app/commands.rs`, `apps/carbide-ui/src/app/helpers.rs`, `apps/carbide-ui/src/app/render_voronoi.rs`, `apps/carbide-ui/src/app/voronoi_helpers.rs`
- **Depends on:** T-002
- **Acceptance criterion:** AC-1 (Voronoi copy/cut/paste with same-lattice formula preservation and cross-lattice literal degradation), AC-2 (marquee `Selection.extra` wired through copy, cut, clear, and collapsed on `begin_edit`), AC-3 (shared `apply_multi_sheet_edits`, `cut_clear_targets`, and `clear_formula_refs` across all 4 lattices).
- **Success criterion (EARS):**
  - **WHEN** the user copies or cuts a single or marquee-selected Voronoi selection (`selection.cells()`), **THEN** `Clipboard` **SHALL** store a `width = 1, height = cells.len() as u32` block (newline-separated TSV) with `SourceLattice::Voronoi` and single `Option<CutOrigin>` (`CutOrigin::Voronoi(cells)` for cut), Escape (`ClearMarquee`) **SHALL** cancel any cut via `self.clipboard.has_cut()`, and pasting onto Voronoi (`PasteMode::Normal`) **SHALL** write raw sources sequentially from cursor within lattice bounds, clear cut origin cells in the same single `Action::Cells` undo step, update `self.voronoi.selection` to `collapse_to(cursor)` + `extra.extend(pasted_coords.into_iter().skip(1))`, and render a dashed cut outline while armed (C-001, C-002).
  - **WHEN** any sheet cuts cells and pastes onto the same or a different lattice (Voronoi ↔ Square/Hex/Triangle) OR uses `PasteMode::ValuesOnly`, **THEN** cross-lattice / `ValuesOnly` cells **SHALL** write evaluated literal values via `clipboard.paste_text(...)`, and both cut-origin clearing and destination writes **SHALL** execute atomically in one `Action::Cells` undo step via `apply_multi_sheet_edits` (C-001).
  - **WHEN** `clear_active` runs on a Voronoi marquee selection, **THEN** every cell in `self.voronoi.selection.cells()` **SHALL** be cleared in one undoable `Action::Cells` step; **WHEN** `begin_edit` runs on Voronoi, **THEN** `self.voronoi.selection` **SHALL** collapse to `cursor`; **WHEN** `commit_edit` or `Command::Cancel` runs, **THEN** `clear_formula_refs` **SHALL** clear `formula_highlight` and `formula_drag` across all four sheets (C-005).
- **Notes:** Extract pure `voronoi_paste_targets` helper in `voronoi_helpers.rs` and `dedup_sheet_targets` in `helpers.rs`, with unit tests covering same-lattice, cross-lattice, `ValuesOnly`, out-of-bounds truncation, non-contiguous cut clearing, and multi-sheet deduplication.

### T-004: Multi-lattice Toggle/Button widget parity, wasm32 dialog filter fix, and verification tests
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- **Touches:** `apps/carbide-ui/src/app/commands.rs`, `apps/carbide-ui/src/app/helpers.rs`, `apps/carbide-ui/src/app/io.rs`, `apps/carbide-ui/src/app/voronoi_helpers.rs`
- **Depends on:** T-003
- **Acceptance criterion:** AC-2 (Voronoi marquee `Selection.extra` wired through widget toggle/button pipelines), AC-3 (shared discriminant-safe `toggle_widget_kind` helper replaces duplicated toggle/button dispatch across Square, Hex, Triangle, and Voronoi, covered by unit tests). Also fixes wasm32 `.crbd`/`.tscl` file-dialog filter parity.
- **Success criterion (EARS):**
  - **WHEN** `toggle_widget_cells` (Toggle) or `toggle_button_cells` (Button) runs on Square, Hex, Triangle, or Voronoi (including a Voronoi marquee selection), **THEN** the shared `toggle_widget_kind` helper (matching via `std::mem::discriminant`, C-005) **SHALL** toggle every selected cell off if all already have that widget kind, or set all to that widget kind otherwise; Slider and ProgressBar **SHALL** remain Square-only per ADR-006.
  - **WHEN** `apps/carbide-ui/src/app/io.rs` is compiled for `wasm32-unknown-unknown`, **THEN** `handle_save` **SHALL** filter on `&["crbd"]` and `handle_open` **SHALL** filter on `&["crbd", "tscl"]` matching native ADR-015 behavior, and `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` **SHALL** exit 0 with zero warnings.
  - **WHEN** all unit tests, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, line-bound checks (`max <= 1000`), and allow-attribute checks run across both workspaces, **THEN** all checks **SHALL** pass with zero warnings and zero `#[allow(...)]` attributes under `apps/carbide-ui/src/app/`.
- **Notes:** Place new pure helper unit tests in `helpers.rs` / `voronoi_helpers.rs` / `clipboard.rs` / `state_io.rs` as appropriate so every file under `apps/carbide-ui/src/app/` stays well below 1,000 lines.
