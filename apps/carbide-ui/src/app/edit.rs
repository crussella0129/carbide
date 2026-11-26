//! In-cell editing, clipboard copy/cut/paste, undo/redo, fill, and header resize/autofit.

use carbide_core::SheetId;
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use eframe::egui;

use crate::clipboard::{Clipboard, CopiedCell, PasteMode, SourceLattice};
use crate::grid;
use crate::selection::{FillDir, HexSelection, SquareSelection, TriangleSelection};

use super::helpers::{
    commit_source, dedup_sheet_targets, dedup_targets, hex_address, hex_in_view, natural_text,
    triangle_address, triangle_in_view,
};
use super::types::{Action, ActiveSheet, CellEdit, EditState, FillAxis, Resize};
use super::voronoi_helpers::{voronoi_address, voronoi_paste_targets};
use super::{CarbideApp, COLS, ROWS};

impl CarbideApp {
    pub(super) fn begin_edit(&mut self, replace_with: Option<char>) {
        // Stage Mode locks cells against direct editing — widgets and
        // interactive cells stay live, but a click or F2 on a regular
        // cell should not open the editor.
        if self.stage_mode {
            return;
        }
        // An edit acts on a single cell — collapse any range to its cursor.
        match self.active {
            ActiveSheet::Square => {
                let cursor = self.square.selection.cursor;
                self.square.selection.collapse_to(cursor);
            }
            ActiveSheet::Hex => {
                let cursor = self.hex.selection.cursor;
                self.hex.selection.collapse_to(cursor);
            }
            ActiveSheet::Triangle => {
                let cursor = self.triangle.selection.cursor;
                self.triangle.selection.collapse_to(cursor);
            }
            ActiveSheet::Voronoi => {
                let cursor = self.voronoi.selection.cursor;
                self.voronoi.selection.collapse_to(cursor);
            }
        }
        let buffer = match replace_with {
            Some(ch) => ch.to_string(),
            None => self.active_cell_source(),
        };
        self.edit = Some(EditState {
            buffer,
            fresh: true,
        });
    }

    /// Apply a batch of cell writes as one undoable step. Each target is
    /// an `(address, new source)`; a target that doesn't actually change
    /// the cell is dropped, so an undo step never holds a no-op.
    pub(super) fn apply_edits(&mut self, sheet: SheetId, targets: Vec<(String, Option<String>)>) {
        let mut edits = Vec::new();
        for (addr, after) in dedup_targets(targets) {
            let before = self.engine.get_cell(sheet, &addr).and_then(|s| s.source);
            if before == after {
                continue;
            }
            let _ = self.engine.set_cell(sheet, &addr, after.as_deref());
            edits.push(CellEdit {
                sheet,
                addr,
                before,
                after,
            });
        }
        if !edits.is_empty() {
            self.history.record(Action::Cells(edits));
        }
    }

    /// Apply a batch of cell writes across one or more sheets as a single
    /// atomic undoable step (e.g. cross-sheet cut/paste clearing the origin
    /// sheet and writing the destination sheet together).
    pub(super) fn apply_multi_sheet_edits(
        &mut self,
        targets: Vec<(SheetId, String, Option<String>)>,
    ) {
        let mut edits = Vec::new();
        for (sheet, addr, after) in dedup_sheet_targets(targets) {
            let before = self.engine.get_cell(sheet, &addr).and_then(|s| s.source);
            if before == after {
                continue;
            }
            let _ = self.engine.set_cell(sheet, &addr, after.as_deref());
            edits.push(CellEdit {
                sheet,
                addr,
                before,
                after,
            });
        }
        if !edits.is_empty() {
            self.history.record(Action::Cells(edits));
        }
    }

    /// Clear active formula-reference highlights and drags across all four
    /// sheets when an edit is committed or cancelled.
    pub(super) fn clear_formula_refs(&mut self) {
        self.square.formula_highlight = None;
        self.square.formula_drag = None;
        self.hex.formula_highlight = None;
        self.hex.formula_drag = None;
        self.triangle.formula_highlight = None;
        self.triangle.formula_drag = None;
        self.voronoi.formula_highlight = None;
        self.voronoi.formula_drag = None;
    }

    /// Write the in-progress edit to the active sheet and leave editing
    /// mode. An all-whitespace buffer clears the cell.
    pub(super) fn commit_edit(&mut self) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        self.clear_formula_refs();
        let source = commit_source(&edit.buffer);
        let (sheet, addr) = self.active_target();
        self.apply_edits(sheet, vec![(addr, source)]);
    }

    /// Clear the selection on whichever sheet is active — including all
    /// marquee-selected cells (`Selection.extra`) on Voronoi.
    pub(super) fn clear_active(&mut self) {
        let (sheet, targets) = match self.active {
            ActiveSheet::Square => {
                let targets = self
                    .square
                    .selection
                    .cells()
                    .into_iter()
                    .map(|(c, r)| (grid::cell_address(c, r), None))
                    .collect();
                (self.square.sheet_id, targets)
            }
            ActiveSheet::Hex => {
                let targets = self
                    .hex
                    .selection
                    .cells()
                    .into_iter()
                    .map(|c| (hex_address(c), None))
                    .collect();
                (self.hex.sheet_id, targets)
            }
            ActiveSheet::Triangle => {
                let targets = self
                    .triangle
                    .selection
                    .cells()
                    .into_iter()
                    .map(|c| (triangle_address(c), None))
                    .collect();
                (self.triangle.sheet_id, targets)
            }
            ActiveSheet::Voronoi => {
                let targets = self
                    .voronoi
                    .selection
                    .cells()
                    .into_iter()
                    .map(|c| (voronoi_address(c), None))
                    .collect();
                (self.voronoi.sheet_id, targets)
            }
        };
        self.apply_edits(sheet, targets);
    }

    /// Read a cell into a `CopiedCell` — its source plus its value as a
    /// re-typeable literal (the fallback a cross-lattice paste writes).
    fn copied_cell(&self, sheet: SheetId, addr: &str) -> CopiedCell {
        match self.engine.get_cell(sheet, addr) {
            Some(snapshot) => {
                let value = natural_text(snapshot.value);
                CopiedCell {
                    source: snapshot.source,
                    value: (!value.is_empty()).then_some(value),
                }
            }
            None => CopiedCell::default(),
        }
    }

    /// Copy the selected range into the in-app clipboard and push a
    /// tab-separated grid of displayed values to the system clipboard.
    /// Works on either sheet — a hex range is a `q × r` block, so it
    /// captures into the same rectangular clipboard.
    pub(super) fn copy_selection(&mut self, ctx: &egui::Context) {
        match self.active {
            ActiveSheet::Square => {
                let ((min_c, min_r), (max_c, max_r)) = self.square.selection.bounds();
                let (width, height) = self.square.selection.dimensions();
                let mut cells = Vec::with_capacity((width * height) as usize);
                let mut tsv = String::new();
                for r in min_r..=max_r {
                    for c in min_c..=max_c {
                        if c > min_c {
                            tsv.push('\t');
                        }
                        tsv.push_str(&self.cell_text(c, r));
                        cells.push(
                            self.copied_cell(self.square.sheet_id, &grid::cell_address(c, r)),
                        );
                    }
                    tsv.push('\n');
                }
                self.clipboard = Clipboard::capture(width, height, cells, SourceLattice::Square);
                ctx.copy_text(tsv);
            }
            ActiveSheet::Hex => {
                let (min, max) = self.hex.selection.bounds();
                let (min_q, min_r, max_q, max_r) = (min.q, min.r, max.q, max.r);
                let (width, height) = self.hex.selection.dimensions();
                let mut cells = Vec::new();
                let mut tsv = String::new();
                for r in min_r..=max_r {
                    for q in min_q..=max_q {
                        if q > min_q {
                            tsv.push('\t');
                        }
                        let coord = HexCoord::new(q, r);
                        tsv.push_str(&self.hex_cell_text(coord));
                        cells.push(self.copied_cell(self.hex.sheet_id, &hex_address(coord)));
                    }
                    tsv.push('\n');
                }
                self.clipboard = Clipboard::capture(width, height, cells, SourceLattice::Hex);
                ctx.copy_text(tsv);
            }
            ActiveSheet::Triangle => {
                let (min, max) = self.triangle.selection.bounds();
                let (width, height) = self.triangle.selection.dimensions();
                let mut cells = Vec::with_capacity((width * height) as usize);
                let mut tsv = String::new();
                for row in min.row..=max.row {
                    for col in min.col..=max.col {
                        if col > min.col {
                            tsv.push('\t');
                        }
                        let coord = TriCoord::new(col, row);
                        tsv.push_str(&self.triangle_cell_text(coord));
                        cells.push(
                            self.copied_cell(self.triangle.sheet_id, &triangle_address(coord)),
                        );
                    }
                    tsv.push('\n');
                }
                self.clipboard = Clipboard::capture(width, height, cells, SourceLattice::Triangle);
                ctx.copy_text(tsv);
            }
            ActiveSheet::Voronoi => {
                let selected = self.voronoi.selection.cells();
                let height = selected.len() as u32;
                let mut cells = Vec::with_capacity(selected.len());
                let mut tsv = String::new();
                for coord in selected {
                    tsv.push_str(&self.voronoi_cell_text(coord));
                    tsv.push('\n');
                    cells.push(self.copied_cell(self.voronoi.sheet_id, &voronoi_address(coord)));
                }
                self.clipboard = Clipboard::capture(1, height, cells, SourceLattice::Voronoi);
                ctx.copy_text(tsv);
            }
        }
    }

    /// Cut the selected range — captures it like a copy, then arms the
    /// range so the next paste clears it atomically (same-sheet or
    /// cross-sheet). Works across all four lattices.
    pub(super) fn cut_selection(&mut self, ctx: &egui::Context) {
        self.copy_selection(ctx);
        match self.active {
            ActiveSheet::Square => {
                let ((c, r), _) = self.square.selection.bounds();
                self.clipboard.mark_as_cut((c as i32, r as i32));
            }
            ActiveSheet::Hex => {
                let (min, _) = self.hex.selection.bounds();
                self.clipboard.mark_as_cut((min.q, min.r));
            }
            ActiveSheet::Triangle => {
                let (min, _) = self.triangle.selection.bounds();
                self.clipboard.mark_as_cut((min.col, min.row));
            }
            ActiveSheet::Voronoi => {
                self.clipboard
                    .mark_as_voronoi_cut(self.voronoi.selection.cells());
            }
        }
    }

    /// Build the `(sheet, addr, None)` clear edits for an armed cut across
    /// any lattice, queued ahead of paste writes so overlapping cells are
    /// overwritten and cross-sheet cuts stay atomic in one undo step.
    fn cut_clear_targets(&self) -> Vec<(SheetId, String, Option<String>)> {
        let mut clears = Vec::new();
        let (width, height) = self.clipboard.dimensions();
        match self.clipboard.source() {
            SourceLattice::Square => {
                if let Some((oc, or)) = self.clipboard.cut_origin() {
                    for j in 0..height {
                        for i in 0..width {
                            let addr = grid::cell_address(oc as u32 + i, or as u32 + j);
                            clears.push((self.square.sheet_id, addr, None));
                        }
                    }
                }
            }
            SourceLattice::Hex => {
                if let Some((oq, or)) = self.clipboard.cut_origin() {
                    for j in 0..height {
                        for i in 0..width {
                            let coord = HexCoord::new(oq + i as i32, or + j as i32);
                            clears.push((self.hex.sheet_id, hex_address(coord), None));
                        }
                    }
                }
            }
            SourceLattice::Triangle => {
                if let Some((oc, or)) = self.clipboard.cut_origin() {
                    for j in 0..height {
                        for i in 0..width {
                            let coord = TriCoord::new(oc + i as i32, or + j as i32);
                            clears.push((self.triangle.sheet_id, triangle_address(coord), None));
                        }
                    }
                }
            }
            SourceLattice::Voronoi => {
                if let Some(coords) = self.clipboard.cut_voronoi_cells() {
                    for &coord in coords {
                        clears.push((self.voronoi.sheet_id, voronoi_address(coord), None));
                    }
                }
            }
        }
        clears
    }

    /// Paste the clipboard block with its top-left at the active cell,
    /// recorded as one atomic undo step (including clearing any armed cut
    /// origin, whether on the same sheet or another lattice). Pasting onto
    /// the same kind of sheet preserves sources; pasting across lattices or
    /// with `ValuesOnly` writes evaluated literals.
    pub(super) fn paste(&mut self, mode: PasteMode) {
        if self.clipboard.is_empty() {
            return;
        }
        let (width, height) = self.clipboard.dimensions();
        let mut targets = self.cut_clear_targets();
        match self.active {
            ActiveSheet::Square => {
                let (target_c, target_r) = self.square.selection.cursor;
                for (rel_c, rel_r, cell) in self.clipboard.entries() {
                    let c = target_c + rel_c;
                    let r = target_r + rel_r;
                    if c >= COLS || r >= ROWS {
                        continue;
                    }
                    let source = self.clipboard.paste_text(cell, SourceLattice::Square, mode);
                    targets.push((self.square.sheet_id, grid::cell_address(c, r), source));
                }
                self.apply_multi_sheet_edits(targets);
                // Select the pasted block, the active cell at its top-left.
                let end_c = (target_c + width - 1).min(COLS - 1);
                let end_r = (target_r + height - 1).min(ROWS - 1);
                self.square.selection =
                    SquareSelection::from_anchor_cursor((end_c, end_r), (target_c, target_r));
            }
            ActiveSheet::Hex => {
                let cursor = self.hex.selection.cursor;
                for (rel_c, rel_r, cell) in self.clipboard.entries() {
                    let coord = HexCoord::new(cursor.q + rel_c as i32, cursor.r + rel_r as i32);
                    if !hex_in_view(coord) {
                        continue;
                    }
                    let source = self.clipboard.paste_text(cell, SourceLattice::Hex, mode);
                    targets.push((self.hex.sheet_id, hex_address(coord), source));
                }
                self.apply_multi_sheet_edits(targets);
                // Select the pasted block, the cursor at its origin.
                let far = HexCoord::new(cursor.q + width as i32 - 1, cursor.r + height as i32 - 1);
                self.hex.selection = HexSelection::from_anchor_cursor(far, cursor);
            }
            ActiveSheet::Triangle => {
                let cursor = self.triangle.selection.cursor;
                for (rel_c, rel_r, cell) in self.clipboard.entries() {
                    let coord = TriCoord::new(cursor.col + rel_c as i32, cursor.row + rel_r as i32);
                    if !triangle_in_view(coord) {
                        continue;
                    }
                    let source = self
                        .clipboard
                        .paste_text(cell, SourceLattice::Triangle, mode);
                    targets.push((self.triangle.sheet_id, triangle_address(coord), source));
                }
                self.apply_multi_sheet_edits(targets);
                let far = TriCoord::new(
                    cursor.col + width as i32 - 1,
                    cursor.row + height as i32 - 1,
                );
                self.triangle.selection = TriangleSelection::from_anchor_cursor(far, cursor);
            }
            ActiveSheet::Voronoi => {
                let cursor = self.voronoi.selection.cursor;
                let lattice_len = self.voronoi_lattice.len() as u32;
                let (vor_targets, pasted_coords) =
                    voronoi_paste_targets(&self.clipboard, cursor, lattice_len, mode);
                for (addr, source) in vor_targets {
                    targets.push((self.voronoi.sheet_id, addr, source));
                }
                self.apply_multi_sheet_edits(targets);
                self.voronoi.selection.collapse_to(cursor);
                self.voronoi
                    .selection
                    .extra
                    .extend(pasted_coords.into_iter().skip(1));
            }
        }
        // A paste consumes the cut — the clipboard reverts to a copy.
        self.clipboard.consume_cut();
    }

    /// Revert the most recent action — cell content or formatting. The
    /// engine recomputes any dependents.
    pub(super) fn undo(&mut self) {
        let Some(action) = self.history.undo() else {
            return;
        };
        match action {
            Action::Cells(edits) => {
                for edit in &edits {
                    let _ = self
                        .engine
                        .set_cell(edit.sheet, &edit.addr, edit.before.as_deref());
                }
            }
            Action::Formats(edits) => {
                for edit in &edits {
                    self.set_cell_format(edit.cell, edit.before.clone());
                }
            }
        }
    }

    /// Re-apply the most recently undone action.
    pub(super) fn redo(&mut self) {
        let Some(action) = self.history.redo() else {
            return;
        };
        match action {
            Action::Cells(edits) => {
                for edit in &edits {
                    let _ = self
                        .engine
                        .set_cell(edit.sheet, &edit.addr, edit.after.as_deref());
                }
            }
            Action::Formats(edits) => {
                for edit in &edits {
                    self.set_cell_format(edit.cell, edit.after.clone());
                }
            }
        }
    }

    /// Fill the selection's leading edge across the rest of it — Ctrl+D
    /// (down) / Ctrl+R (right), on either sheet. Sources copy verbatim
    /// and the whole fill is a single undo step.
    pub(super) fn fill(&mut self, dir: FillDir) {
        let (sheet, targets) = match self.active {
            ActiveSheet::Square => {
                let targets = self
                    .square
                    .selection
                    .fill_targets(dir)
                    .into_iter()
                    .map(|(target, from)| {
                        let source = self
                            .engine
                            .get_cell(self.square.sheet_id, &grid::cell_address(from.0, from.1))
                            .and_then(|s| s.source);
                        (grid::cell_address(target.0, target.1), source)
                    })
                    .collect();
                (self.square.sheet_id, targets)
            }
            ActiveSheet::Hex => {
                let targets = self
                    .hex
                    .selection
                    .fill_targets(dir)
                    .into_iter()
                    .map(|(target, from)| {
                        let source = self
                            .engine
                            .get_cell(self.hex.sheet_id, &hex_address(from))
                            .and_then(|s| s.source);
                        (hex_address(target), source)
                    })
                    .collect();
                (self.hex.sheet_id, targets)
            }
            ActiveSheet::Triangle => {
                let targets: Vec<(String, Option<String>)> = self
                    .triangle
                    .selection
                    .fill_targets(dir)
                    .into_iter()
                    .map(|(target, from)| {
                        let source = self
                            .engine
                            .get_cell(self.triangle.sheet_id, &triangle_address(from))
                            .and_then(|s| s.source);
                        (triangle_address(target), source)
                    })
                    .collect();
                (self.triangle.sheet_id, targets)
            }
            // Voronoi has no fill-drag geometry — return an empty edit
            // set so the caller's `apply_edits` is a no-op.
            ActiveSheet::Voronoi => (self.voronoi.sheet_id, Vec::new()),
        };
        self.apply_edits(sheet, targets);
    }

    /// The visible square of the square sheet's fill handle — small
    /// and centred on the selection's bottom-right corner.
    pub(super) fn fill_handle_visual_rect(&self, origin: egui::Pos2) -> egui::Rect {
        const SIZE: f32 = 8.0;
        let (_, (max_c, max_r)) = self.square.selection.bounds();
        let cell = self.metrics.cell_rect(origin, max_c, max_r);
        let center = cell.right_bottom();
        egui::Rect::from_center_size(center, egui::vec2(SIZE, SIZE))
    }

    /// The generous hit-test zone around the fill handle — visibly the
    /// same corner, but with a wider catchment so near-misses still
    /// register. Matches the way Excel widens the handle hit area.
    pub(super) fn fill_handle_hit_rect(&self, origin: egui::Pos2) -> egui::Rect {
        const HIT: f32 = 16.0;
        let (_, (max_c, max_r)) = self.square.selection.bounds();
        let cell = self.metrics.cell_rect(origin, max_c, max_r);
        let center = cell.right_bottom();
        egui::Rect::from_center_size(center, egui::vec2(HIT, HIT))
    }

    /// Fill the cells of `extended` that lie outside `original`,
    /// using each fill lane's seed values from `original`. For each
    /// lane (a column when filling down, a row when filling right),
    /// [`grid::fill_lane`] extends an arithmetic progression when
    /// every non-empty seed parses as a number — otherwise it repeats
    /// the seed pattern. Writes that would equal the cell's existing
    /// source are skipped so they don't pollute the undo history.
    pub(super) fn fill_handle_apply(
        &mut self,
        original: ((u32, u32), (u32, u32)),
        extended: ((u32, u32), (u32, u32)),
    ) {
        let ((oc0, or0), (oc1, or1)) = original;
        let (_, (nc1, nr1)) = extended;
        // The locked drag axis already pinned the cursor to a single
        // dimension, so `extended` only grows along one axis here.
        let mode = if nr1 > or1 {
            FillAxis::Down
        } else if nc1 > oc1 {
            FillAxis::Right
        } else {
            // Selection shrunk or stayed put — no fill needed. The
            // drag-stopped path also short-circuits on extended ==
            // original, but defend against odd cases.
            return;
        };
        let format_num = |n: f64| {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                (n as i64).to_string()
            } else {
                format!("{n}")
            }
        };
        let mut targets = Vec::new();
        let push = |this: &Self,
                    targets: &mut Vec<(String, Option<String>)>,
                    cell: (u32, u32),
                    new_value: Option<String>| {
            let addr = grid::cell_address(cell.0, cell.1);
            let current = this
                .engine
                .get_cell(this.square.sheet_id, &addr)
                .and_then(|s| s.source);
            // Skip identical writes — keeps the undo history clean
            // when the drag re-traces ground that already matches the
            // pattern.
            if current != new_value {
                targets.push((addr, new_value));
            }
        };
        match mode {
            FillAxis::Down => {
                let extend_count = (nr1 - or1) as usize;
                for c in oc0..=oc1 {
                    let seed: Vec<Option<String>> = (or0..=or1)
                        .map(|r| {
                            self.engine
                                .get_cell(self.square.sheet_id, &grid::cell_address(c, r))
                                .and_then(|s| s.source)
                        })
                        .collect();
                    let extension = grid::fill_lane(&seed, extend_count, format_num);
                    for (i, value) in extension.into_iter().enumerate() {
                        let r = or1 + 1 + i as u32;
                        push(self, &mut targets, (c, r), value);
                    }
                }
            }
            FillAxis::Right => {
                let extend_count = (nc1 - oc1) as usize;
                for r in or0..=or1 {
                    let seed: Vec<Option<String>> = (oc0..=oc1)
                        .map(|c| {
                            self.engine
                                .get_cell(self.square.sheet_id, &grid::cell_address(c, r))
                                .and_then(|s| s.source)
                        })
                        .collect();
                    let extension = grid::fill_lane(&seed, extend_count, format_num);
                    for (i, value) in extension.into_iter().enumerate() {
                        let c = oc1 + 1 + i as u32;
                        push(self, &mut targets, (c, r), value);
                    }
                }
            }
        }
        if !targets.is_empty() {
            self.apply_edits(self.square.sheet_id, targets);
        }
    }

    /// Resize the header border under an in-progress drag. The headers
    /// are both frozen, so the hit-tests use their floating origins —
    /// `col_hdr_origin` for column borders, `row_hdr_origin` for row.
    ///
    /// The hit-test runs on the **press position** captured in
    /// [`Self::press_pos`] rather than the pointer's position at the
    /// moment `drag_started` fires. egui's drag threshold lets the
    /// pointer travel 5+ pixels before that event arrives — far enough
    /// to leave the border's grab zone and have the click be misread
    /// as a header-select drag.
    pub(super) fn handle_resize(
        &mut self,
        response: &egui::Response,
        col_hdr_origin: egui::Pos2,
        row_hdr_origin: egui::Pos2,
    ) {
        if response.drag_started() {
            if let Some(p) = self.press_pos.or_else(|| response.interact_pointer_pos()) {
                self.resizing = self
                    .metrics
                    .col_border_at(col_hdr_origin, p, COLS)
                    .map(Resize::Column)
                    .or_else(|| {
                        self.metrics
                            .row_border_at(row_hdr_origin, p, ROWS)
                            .map(Resize::Row)
                    });
            }
        }
        if response.dragged() {
            let delta = response.drag_delta();
            match self.resizing {
                Some(Resize::Column(c)) => {
                    let w = self.metrics.col_width(c) + delta.x;
                    self.metrics.set_col_width(c, w);
                }
                Some(Resize::Row(r)) => {
                    let h = self.metrics.row_height(r) + delta.y;
                    self.metrics.set_row_height(r, h);
                }
                None => {}
            }
        }
        if response.drag_stopped() {
            self.resizing = None;
        }
    }

    /// Set a column's width to fit its widest cell's rendered text —
    /// triggered by a double-click on the column's resize border.
    pub(super) fn autofit_column(&mut self, col: u32, painter: &egui::Painter) {
        let widths = (0..ROWS).map(|row| {
            let text = self.cell_text(col, row);
            if text.is_empty() {
                0.0
            } else {
                let pts = self.square.formats.get((col, row)).font_size.points();
                painter
                    .layout_no_wrap(text, egui::FontId::proportional(pts), egui::Color32::BLACK)
                    .size()
                    .x
            }
        });
        // 5.0 is the cell text's left/right inset in `draw_cell_text`.
        let width = grid::fit_extent(widths, 2.0 * 5.0, grid::MIN_COL_W);
        self.metrics.set_col_width(col, width);
    }

    /// Set a row's height to fit its tallest cell's rendered text —
    /// triggered by a double-click on the row's resize border. Each
    /// cell is measured at its own font size.
    pub(super) fn autofit_row(&mut self, row: u32, painter: &egui::Painter) {
        let heights = (0..COLS).map(|col| {
            let text = self.cell_text(col, row);
            if text.is_empty() {
                0.0
            } else {
                let pts = self.square.formats.get((col, row)).font_size.points();
                painter
                    .layout_no_wrap(text, egui::FontId::proportional(pts), egui::Color32::BLACK)
                    .size()
                    .y
            }
        });
        // 5.0 is the cell text's top/bottom inset in `draw_cell_text`.
        let height = grid::fit_extent(heights, 2.0 * 5.0, grid::MIN_ROW_H);
        self.metrics.set_row_height(row, height);
    }

    /// The square cell under a pointer position, if any.
    pub(super) fn cell_under(
        &self,
        response: &egui::Response,
        origin: egui::Pos2,
        header_x: f32,
        header_y: f32,
    ) -> Option<(u32, u32)> {
        response.interact_pointer_pos().and_then(|p| {
            self.metrics
                .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
        })
    }
}
