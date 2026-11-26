# Sprint 14 Research Report — Lattice UI Parity and Voronoi Operational Unification

## Intents Reviewed
- [INT-0003 — Lattice UI parity and Voronoi operational unification](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) (state: `proposed`)

## Summary
Sprint 13 (`INT-0002`) decomposed `apps/carbide-ui/src/app.rs` into 15 focused submodules under `apps/carbide-ui/src/app/`, giving Sprint 14 clean boundaries to complete `INT-0003`: bringing Voronoi to full operational parity with square, hex, and triangle (formatting, notes, copy/cut/paste, and multi-cell marquee `Selection.extra` operations), unifying duplicated per-lattice dispatch/history paths, and fixing the pre-existing wasm32 `.crbd`/`.tscl` dialog filter inconsistency discovered during the Sprint 13 review.

## Findings

### 1. Formatting & Undo History Duplication
- `CarbideApp` already stores `voronoi: Sheet<VoronoiCoord>`, so `self.voronoi.formats: FormatMap<VoronoiCoord>` exists today and `FormatMap<K>` already derives `Serialize`/`Deserialize` for any `K: Serialize + Deserialize + Eq + Hash + Copy` (`apps/carbide-ui/src/format.rs:382-420`, and `VoronoiCoord` already implements those traits).
- Yet Voronoi formatting is unwired in 5 places:
  1. `apps/carbide-ui/src/app/cells.rs:291-306`: `format_range` has `ActiveSheet::Voronoi => {}` and `ActiveSheet::Triangle` bypasses undo/coalescing entirely.
  2. `apps/carbide-ui/src/app/cells.rs:404-427`: `toggle_range` returns `false` for Voronoi.
  3. `apps/carbide-ui/src/app/mod.rs:397`: Ribbon passes `CellFormat::default()` on Voronoi instead of `self.voronoi.formats.get(self.voronoi.selection.cursor)`.
  4. `apps/carbide-ui/src/app/render_voronoi.rs:155-191`: Polygon fills ignore `fmt.fill` and centroid text ignores `fmt.text_color`, `fmt.font_size`, `fmt.bold`, `fmt.italic`, `fmt.strikethrough`, `fmt.underline`, `fmt.negative_red`, and `fmt.number` (via missing `voronoi_cell_value` / `voronoi_effective_format`).
  5. `apps/carbide-ui/src/state_io.rs:46-76` & `apps/carbide-ui/src/app/io.rs:212-306`: `UiSnapshot` lacks `voronoi_formats`.
- **Unification opportunity (AC-3):** `types.rs:289-312` duplicates `FormatEdit` (square) vs `HexFormatEdit` (hex), `Action::Formats` vs `Action::HexFormats`, `helpers.rs:475-513` duplicates `merge_format_edits` vs `merge_hex_format_edits`, `mod.rs:210-214` duplicates `last_format_time`/`last_format_cells` vs `last_hex_format_*`, and `cells.rs:311-399` duplicates `format_square_range` vs `format_hex_range`. Replacing `FormatEdit.cell` with `CellId` collapses all four lattices (Square, Hex, Triangle, Voronoi) into ONE `Action::Formats(Vec<FormatEdit>)`, ONE `merge_format_edits`, ONE coalescing tracker, and ONE `format_range` / `toggle_range` pipeline — giving both Triangle AND Voronoi full undo/redo and colour-drag coalescing in less code than before.

### 2. Notes Parity & Multi-Lattice `OpenNote`
- `CarbideApp` has `notes`, `hex_notes`, `triangle_notes` (`apps/carbide-ui/src/app/mod.rs:252-257`), missing `voronoi_notes: NoteMap<VoronoiCoord>`.
- `apps/carbide-ui/src/app/dialogs.rs:415-425`: `set_note` already formats `CellId::Voronoi` in `note_window` (`dialogs.rs:385`) but drops writes with `CellId::Voronoi(_) => {}`.
- `apps/carbide-ui/src/app/commands.rs:304-309`: `RibbonAction::OpenNote` hardcodes `CellId::Square(self.square.selection.cursor)` regardless of active sheet! Extracting `active_cell_id(&self) -> CellId`, `note_text(&self, cell: CellId) -> &str`, and `open_note(&mut self, cell: CellId)` fixes `OpenNote` across all four sheets and eliminates duplicated context-menu note-opening code.
- `render_voronoi.rs` has no note marker dot, hover tooltip (error/note), or right-click context menu (unlike `render_hex.rs:85-90, 226-309`). Adding a note marker dot, hover tooltip (error detail or note), and context menu (Copy / Cut / Paste / Paste values / Clear / Edit note…) brings Voronoi into visual and interaction parity.
- `state_io.rs:80-107` uses `vec_pair_for!` for notes — adding `vec_pair_for!(voronoi_notes, carbide_tess::voronoi::VoronoiCoord)` and `#[serde(default, with = "vec_pair::voronoi_notes")] pub voronoi_notes: HashMap<VoronoiCoord, String>` preserves `.crbd` v2 backward compatibility (`FORMAT_VERSION` unchanged at `2`).

### 3. Voronoi Multi-Cell Marquee (`Selection.extra`) & Clipboard Unification
- `Selection<VoronoiCoord>::cells()` returns `[anchor]` plus deduplicated `extra` cells in deterministic order (`apps/carbide-ui/src/selection.rs:373-380`). ADR-013 temporarily scope-capped operations to `primary_cells()`, which only returns `[anchor]` on Voronoi.
- Moving Voronoi operational paths to `selection.cells()`:
  - **Format & Toggle:** `active_selection_cells()` returns `selection.cells()` on Voronoi (and `primary_cells()` on Square/Hex/Triangle, which have empty `extra`), so Bold/Italic/Fill/etc. apply to every marquee-selected Voronoi cell in one undo step.
  - **Clear (`edit.rs:126-129`):** Currently clears only `cursor`; switching to `self.voronoi.selection.cells()` clears all marquee-selected cells in one undo step.
  - **Begin Edit (`edit.rs:42`):** Currently `ActiveSheet::Voronoi => {}` leaves stale marquee `extra` highlights when starting an edit; calling `collapse_to(cursor)` matches Square/Hex/Triangle.
  - **Status Bar Hint (`mod.rs:480-484`):** Currently displays `self.voronoi.selection.extra.len() + 1`, which overcounts by 1 if `cursor` is inside `extra`. Using `self.voronoi.selection.cells().len()` fixes the count.
  - **Widgets (`commands.rs:339-375`):** `toggle_widget_cells` and `toggle_button_cells` early-return unless `self.active == ActiveSheet::Square`, even though Hex, Triangle, and Voronoi all have `Widgets<K>` supporting Toggle and Button! Extracting a generic `toggle_widget_kind<K>(widgets, cells, ...)` enables Toggle and Button ribbon actions across all four lattices (using `selection.cells()` on Voronoi, while keeping Slider and ProgressBar square-only per ADR-006).
- **Clipboard (`clipboard.rs`, `edit.rs`, `render_voronoi.rs`):**
  - `SourceLattice` (`clipboard.rs:44-49`) lacks `Voronoi`.
  - Voronoi marquee selections can be non-contiguous seed indices (e.g. `V(0)` and `V(3)`), so a rectangular `cut_origin: Option<(i32, i32)>` cannot represent an arbitrary Voronoi cut. Adding `cut_voronoi: Option<Vec<VoronoiCoord>>` (with `mark_as_voronoi_cut`, `cut_voronoi_cells`, cleared by `consume_cut` and `has_cut()`) cleanly tracks exact cut Voronoi cells for both clearing on paste and rendering a dashed cut outline in `render_voronoi.rs`.
  - Copy captures `self.voronoi.selection.cells()` as a `1 × N` block (`SourceLattice::Voronoi`) with TSV export; same-lattice `PasteMode::Normal` preserves formulas (`V(...)`), cross-lattice or `PasteMode::ValuesOnly` writes evaluated values via `clipboard.paste_text(...)`. Pasting onto Voronoi writes flattened clipboard entries starting at `cursor` (`VoronoiCoord(cursor.0 + i)`) bounded by `self.voronoi_lattice.len() as u32`.

### 4. Wasm32 File-Dialog Filter Parity Bug
- `apps/carbide-ui/src/app/io.rs:86` and `io.rs:124` (inside `#[cfg(target_arch = "wasm32")]`) still pass `&["tscl"]` to `rfd::AsyncFileDialog` instead of `&["crbd"]` (save) and `&["crbd", "tscl"]` (open), diverging from native ADR-015 dual-extension behavior (`io.rs:64, 102`). Fixing both restores native/wasm parity.

## Gaps & Risks
- **Existing tests:** `apps/carbide-ui/src/app/tests.rs:757-811` tests `merge_format_edits` and `merge_hex_format_edits` with the old `FormatEdit`/`HexFormatEdit` types. Updating those tests to the unified `FormatEdit { cell: CellId, ... }` preserves full coverage across Square, Hex, Triangle, and Voronoi.
- **File size budget:** Every file under `apps/carbide-ui/src/app/` must remain ≤ 1,000 lines (preserving `INT-0002` hygiene) with zero `#[allow(...)]` attributes and `_f32` suffixes on `egui::Stroke::new` (Rust 1.99 lint).
- **`.crbd` backward compatibility:** `UiSnapshot` already carries `#[serde(default)]` on the struct; adding `voronoi_formats` and `voronoi_notes` with explicit `#[serde(default)]` ensures pre-Sprint-14 v2 snapshots deserialize with empty maps and `FORMAT_VERSION` stays `2`.

## Sources
1. `docs/intents/INT-0003-lattice-ui-parity-and-unification.md` — target intent and acceptance criteria.
2. `apps/carbide-ui/src/app/{types,cells,edit,commands,dialogs,io,mod,render_voronoi,render_hex,helpers,tests}.rs` — current four-lattice dispatch, formatting, notes, widgets, clipboard, and rendering paths.
3. `apps/carbide-ui/src/{clipboard,format,selection,state_io}.rs` — pure models for clipboard, formatting, `Selection.extra`, and `.crbd` `UiSnapshot` serde.
