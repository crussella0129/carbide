# Sprint 14 Unit Test Results

## Scope
Verified all unit-level assertions from [test-plan.md](../sprint-plans/test-plan.md) after `T-004` (`95675d6`) completed.

## Commands Executed
- `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check` → exit 0
- `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` → exit 0 (0 warnings)
- `cargo test --manifest-path apps/carbide-ui/Cargo.toml` → exit 0 (**315 passed**, 0 failed, 0 ignored)
- `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` → exit 0 (0 warnings)
- `wc -l apps/carbide-ui/src/app/*.rs | sort -n` → max **819 lines** (`tests.rs`), all 15 files ≤ 1,000 lines
- `! git grep --untracked -nE '#!?\[[[:space:]]*allow\(' apps/carbide-ui/src/app/` → 0 matches
- `git grep -n 'add_filter' apps/carbide-ui/src/app/io.rs` → native & wasm32 filters both `&["crbd"]` (save) and `&["crbd", "tscl"]` (open)

## Key Verification Cases Passed
- **`verify_unified_format_and_widget_helpers`:**
  - `app::tests::merge_format_edits_keeps_oldest_before_and_newest_after_across_all_lattices` — verified unified `FormatEdit { cell: CellId, ... }` coalescing across `Square`, `Hex`, `Triangle`, and `Voronoi`.
  - `app::helpers::tests::dedup_sheet_targets_keeps_last_write_per_sheet_and_preserves_distinct_sheets` — verified atomic multi-sheet deduplication for cross-sheet cut/paste.
  - `app::helpers::tests::toggle_widget_kind_handles_all_off_mixed_and_all_on_across_lattices` — verified discriminant-safe `toggle_widget_kind` across Square (including parameterized `Slider`), Hex, Triangle, and Voronoi non-contiguous marquee selections (`Selection.extra`).
- **`verify_ui_snapshot_roundtrip_and_compat`:**
  - `state_io::tests::snapshot_round_trips_through_ui_state` — verified `voronoi_formats` and `voronoi_notes` round-trip losslessly alongside existing snapshot fields.
  - `state_io::tests::legacy_v2_snapshot_without_voronoi_formats_or_notes_deserializes_with_defaults` — strict direct `serde_json::from_value::<UiSnapshot>(...).unwrap()` decode verified legacy v2 JSON preserves non-default fields (`active_sheet`, `stage_mode`, `dark_mode`, `square_notes`) while asserting `snap.voronoi_formats.iter().count() == 0` and `snap.voronoi_notes.is_empty()`.
- **`verify_voronoi_clipboard_and_paste`:**
  - `clipboard::tests::voronoi_source_keeps_formula_on_voronoi_and_degrades_cross_lattice`
  - `clipboard::tests::voronoi_non_contiguous_cut_tracks_exact_cells_and_consumes`
  - `app::voronoi_helpers::tests::voronoi_paste_targets_same_lattice_preserves_formulas_and_selection`
  - `app::voronoi_helpers::tests::voronoi_paste_targets_cross_lattice_and_values_only_degrade_to_literals`
  - `app::voronoi_helpers::tests::voronoi_paste_targets_truncates_at_lattice_len`
- **`verify_voronoi_marquee_operations`:**
  - `selection::tests::selection_extra_adds_to_cells_dedup` and `app::helpers::tests::toggle_widget_kind_handles_all_off_mixed_and_all_on_across_lattices` verify `Selection<VoronoiCoord>::cells()` deduplication and marquee fan-out into `toggle_widget_kind` over non-contiguous `Selection.extra` cells.
  - `CarbideApp` method wiring (`active_selection_cells` in `cells.rs`, `clear_active`/`copy_selection`/`cut_selection` in `edit.rs`, `toggle_multi_lattice_widget` in `commands.rs`) is verified by static compilation + code inspection because `CarbideApp::new` requires an `eframe::CreationContext`.
- **`verify_module_hygiene_and_wasm32`:**
  - All 15 files under `apps/carbide-ui/src/app/` are 303–819 lines (`max = 819 <= 1000`).
  - Zero `#![allow(...)]` / `#[allow(...)]` attributes under `apps/carbide-ui/src/app/`.
  - `apps/carbide-ui/src/app/io.rs` wasm32 filters fixed to `&["crbd"]` (save, L86) and `&["crbd", "tscl"]` (open, L124); `wasm32-unknown-unknown` target compiles cleanly.
