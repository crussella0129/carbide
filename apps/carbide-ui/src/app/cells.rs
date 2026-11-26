//! Cell value/source/format accessors, range formatting, and lattice navigation.

use carbide_core::{CellValue, SheetId};
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use carbide_tess::voronoi::VoronoiCoord;

use crate::conditional;
use crate::format::{self, CellFormat};
use crate::grid;
use crate::keymap::{Dir, Mode};

use super::helpers::{
    format_number, hex_address, hex_in_view, hex_jump, hex_step, merge_format_edits,
    merge_hex_format_edits, natural_text, step_square, toggle_target, triangle_address,
    triangle_in_view, triangle_step,
};
use super::types::{Action, ActiveSheet, FormatEdit, HexFormatEdit};
use super::voronoi_helpers::{voronoi_address, voronoi_advance_step};
use super::{CarbideApp, COALESCE_WINDOW, COLS, PAGE_ROWS, ROWS, TRIANGLE_RADIUS};

impl CarbideApp {
    pub(super) fn mode(&self) -> Mode {
        if self.edit.is_some() {
            Mode::Editing
        } else {
            Mode::Navigating
        }
    }

    /// The display text for a square-sheet cell, read from the engine and
    /// rendered under the cell's number format.
    pub(super) fn cell_text(&self, col: u32, row: u32) -> String {
        let addr = grid::cell_address(col, row);
        let Some(snapshot) = self.engine.get_cell(self.square.sheet_id, &addr) else {
            return String::new();
        };
        let number = self.square.formats.get((col, row)).number;
        match snapshot.value {
            CellValue::Number(n) => {
                format::render_number(n, number).unwrap_or_else(|| format_number(n))
            }
            CellValue::Integer(i) => {
                format::render_number(i as f64, number).unwrap_or_else(|| i.to_string())
            }
            other => natural_text(other),
        }
    }

    /// The raw evaluated value of a square-sheet cell — used by
    /// conditional formatting. `Empty` when the cell holds nothing.
    pub(super) fn cell_value(&self, col: u32, row: u32) -> CellValue {
        self.engine
            .get_cell(self.square.sheet_id, &grid::cell_address(col, row))
            .map(|snapshot| snapshot.value)
            .unwrap_or_default()
    }

    /// The raw evaluated value of a hex-sheet cell, for conditional
    /// formatting. `Empty` when the cell holds nothing.
    pub(super) fn hex_cell_value(&self, coord: HexCoord) -> CellValue {
        self.engine
            .get_cell(self.hex.sheet_id, &hex_address(coord))
            .map(|snapshot| snapshot.value)
            .unwrap_or_default()
    }

    /// The effective format for a hex cell — its stored format layered
    /// with the first matching conditional-formatting rule.
    pub(super) fn hex_effective_format(&self, coord: HexCoord) -> CellFormat {
        let base = self.hex.formats.get(coord);
        if self.cond_rules.is_empty() {
            base
        } else {
            conditional::effective_format(&base, &self.hex_cell_value(coord), &self.cond_rules)
        }
    }

    /// The raw source text of a square-sheet cell — empty when the cell
    /// holds nothing. Find and Replace both work on this.
    pub(super) fn cell_source(&self, col: u32, row: u32) -> String {
        self.engine
            .get_cell(self.square.sheet_id, &grid::cell_address(col, row))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The raw source text of a hex-sheet cell — empty when it holds
    /// nothing. Find and Replace use it on the hex sheet.
    pub(super) fn hex_cell_source(&self, coord: HexCoord) -> String {
        self.engine
            .get_cell(self.hex.sheet_id, &hex_address(coord))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The display text for a hex-sheet cell. (The number format is not
    /// yet applied on the hex sheet — that is a follow-on.)
    pub(super) fn hex_cell_text(&self, coord: HexCoord) -> String {
        self.engine
            .get_cell(self.hex.sheet_id, &hex_address(coord))
            .map(|snapshot| natural_text(snapshot.value))
            .unwrap_or_default()
    }

    /// The raw source of a triangle-sheet cell.
    pub(super) fn triangle_cell_source(&self, coord: TriCoord) -> String {
        self.engine
            .get_cell(self.triangle.sheet_id, &triangle_address(coord))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The display text for a triangle-sheet cell.
    pub(super) fn triangle_cell_text(&self, coord: TriCoord) -> String {
        self.engine
            .get_cell(self.triangle.sheet_id, &triangle_address(coord))
            .map(|snapshot| natural_text(snapshot.value))
            .unwrap_or_default()
    }

    /// The raw source of a Voronoi-sheet cell.
    pub(super) fn voronoi_cell_source(&self, coord: VoronoiCoord) -> String {
        self.engine
            .get_cell(self.voronoi.sheet_id, &voronoi_address(coord))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The display text for a Voronoi-sheet cell.
    pub(super) fn voronoi_cell_text(&self, coord: VoronoiCoord) -> String {
        self.engine
            .get_cell(self.voronoi.sheet_id, &voronoi_address(coord))
            .map(|snapshot| natural_text(snapshot.value))
            .unwrap_or_default()
    }

    /// The triangle-sheet cell value used by conditional formatting.
    pub(super) fn triangle_cell_value(&self, coord: TriCoord) -> CellValue {
        self.engine
            .get_cell(self.triangle.sheet_id, &triangle_address(coord))
            .map(|snapshot| snapshot.value)
            .unwrap_or_default()
    }

    /// The effective cell format for a triangle-sheet cell — base
    /// format plus any conditional-rule overrides for the cell's
    /// current value. Cond rules apply uniformly across every sheet.
    pub(super) fn triangle_effective_format(&self, coord: TriCoord) -> CellFormat {
        let base = self.triangle.formats.get(coord);
        if self.cond_rules.is_empty() {
            base
        } else {
            conditional::effective_format(&base, &self.triangle_cell_value(coord), &self.cond_rules)
        }
    }

    /// The raw source of the active square cell (the selection cursor).
    pub(super) fn selected_source(&self) -> String {
        let (col, row) = self.square.selection.cursor;
        self.engine
            .get_cell(self.square.sheet_id, &grid::cell_address(col, row))
            .and_then(|s| s.source)
            .unwrap_or_default()
    }

    /// The raw source of the active cell on whichever sheet is active.
    pub(super) fn active_cell_source(&self) -> String {
        match self.active {
            ActiveSheet::Square => self.selected_source(),
            ActiveSheet::Hex => self
                .engine
                .get_cell(self.hex.sheet_id, &hex_address(self.hex.selection.cursor))
                .and_then(|s| s.source)
                .unwrap_or_default(),
            ActiveSheet::Triangle => self
                .engine
                .get_cell(
                    self.triangle.sheet_id,
                    &triangle_address(self.triangle.selection.cursor),
                )
                .and_then(|s| s.source)
                .unwrap_or_default(),
            ActiveSheet::Voronoi => self
                .engine
                .get_cell(
                    self.voronoi.sheet_id,
                    &voronoi_address(self.voronoi.selection.cursor),
                )
                .and_then(|s| s.source)
                .unwrap_or_default(),
        }
    }

    /// The `(sheet, address)` of the active cell on the active sheet.
    pub(super) fn active_target(&self) -> (SheetId, String) {
        match self.active {
            ActiveSheet::Square => {
                let (col, row) = self.square.selection.cursor;
                (self.square.sheet_id, grid::cell_address(col, row))
            }
            ActiveSheet::Hex => (self.hex.sheet_id, hex_address(self.hex.selection.cursor)),
            ActiveSheet::Triangle => (
                self.triangle.sheet_id,
                triangle_address(self.triangle.selection.cursor),
            ),
            ActiveSheet::Voronoi => (
                self.voronoi.sheet_id,
                voronoi_address(self.voronoi.selection.cursor),
            ),
        }
    }

    /// The active cell's address and source text on the active sheet,
    /// for the formula bar.
    pub(super) fn active_address_and_source(&self) -> (String, String) {
        let addr = match self.active {
            ActiveSheet::Square => {
                let (col, row) = self.square.selection.cursor;
                grid::cell_address(col, row)
            }
            ActiveSheet::Hex => hex_address(self.hex.selection.cursor),
            ActiveSheet::Triangle => triangle_address(self.triangle.selection.cursor),
            ActiveSheet::Voronoi => voronoi_address(self.voronoi.selection.cursor),
        };
        let source = match &self.edit {
            Some(edit) => edit.buffer.clone(),
            None => self.active_cell_source(),
        };
        (addr, source)
    }

    /// The engine values of every cell in the active selection — what the
    /// status bar aggregates.
    pub(super) fn selection_values(&self) -> Vec<CellValue> {
        match self.active {
            ActiveSheet::Square => self
                .square
                .selection
                .cells()
                .into_iter()
                .map(|(c, r)| {
                    self.engine
                        .get_cell(self.square.sheet_id, &grid::cell_address(c, r))
                        .map(|s| s.value)
                        .unwrap_or(CellValue::Empty)
                })
                .collect(),
            ActiveSheet::Hex => self
                .hex
                .selection
                .cells()
                .into_iter()
                .map(|c| {
                    self.engine
                        .get_cell(self.hex.sheet_id, &hex_address(c))
                        .map(|s| s.value)
                        .unwrap_or(CellValue::Empty)
                })
                .collect(),
            ActiveSheet::Triangle => self
                .triangle
                .selection
                .cells()
                .into_iter()
                .map(|c| {
                    self.engine
                        .get_cell(self.triangle.sheet_id, &triangle_address(c))
                        .map(|s| s.value)
                        .unwrap_or(CellValue::Empty)
                })
                .collect(),
            ActiveSheet::Voronoi => self
                .voronoi
                .selection
                .cells()
                .into_iter()
                .map(|c| {
                    self.engine
                        .get_cell(self.voronoi.sheet_id, &voronoi_address(c))
                        .map(|s| s.value)
                        .unwrap_or(CellValue::Empty)
                })
                .collect(),
        }
    }

    /// Apply a formatting change to every cell of the active sheet's
    /// selection, recorded as one undo step. Cells the change leaves
    /// untouched are dropped.
    pub(super) fn format_range(&mut self, edit: impl Fn(&mut CellFormat)) {
        match self.active {
            ActiveSheet::Square => self.format_square_range(edit),
            ActiveSheet::Hex => self.format_hex_range(edit),
            ActiveSheet::Triangle => {
                // Triangle format coalescing lands in a follow-up;
                // apply the edit directly without the same-cell drag
                // merge logic so the basic flow still works.
                let cells = self.triangle.selection.primary_cells();
                for cell in cells {
                    self.triangle.formats.update(cell, |f| edit(f));
                }
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// The square-sheet format apply, with edit-coalescing — a rapid run
    /// of same-cell format edits (a colour-picker drag) collapses into
    /// one undo step.
    fn format_square_range(&mut self, edit: impl Fn(&mut CellFormat)) {
        let cells = self.square.selection.primary_cells();
        let mut edits = Vec::new();
        for cell in cells {
            let before = self.square.formats.get(cell);
            let mut after = before.clone();
            edit(&mut after);
            if before == after {
                continue;
            }
            self.square.formats.update(cell, |f| *f = after.clone());
            edits.push(FormatEdit {
                cell,
                before,
                after,
            });
        }
        if edits.is_empty() {
            return;
        }
        let touched: Vec<(u32, u32)> = edits.iter().map(|e| e.cell).collect();
        // Coalesce with the previous format action when it touched the
        // same cells within COALESCE_WINDOW — a colour-picker drag fires
        // a change every frame, and the whole drag should undo at once.
        let recent = self.frame_time - self.last_format_time < COALESCE_WINDOW;
        if recent && self.last_format_cells == touched {
            match self.history.pop_undo() {
                Some(Action::Formats(prev)) => {
                    let merged = merge_format_edits(prev, edits);
                    self.history.record(Action::Formats(merged));
                }
                Some(other) => {
                    // The top wasn't a format action — restore it.
                    self.history.record(other);
                    self.history.record(Action::Formats(edits));
                }
                None => self.history.record(Action::Formats(edits)),
            }
        } else {
            self.history.record(Action::Formats(edits));
        }
        self.last_format_time = self.frame_time;
        self.last_format_cells = touched;
    }

    /// The hex-sheet format apply, recorded as one undo step. A rapid
    /// run of same-cell edits (a colour drag) coalesces into one step.
    fn format_hex_range(&mut self, edit: impl Fn(&mut CellFormat)) {
        let mut edits = Vec::new();
        for cell in self.hex.selection.primary_cells() {
            let before = self.hex.formats.get(cell);
            let mut after = before.clone();
            edit(&mut after);
            if before == after {
                continue;
            }
            self.hex.formats.update(cell, |f| *f = after.clone());
            edits.push(HexFormatEdit {
                cell,
                before,
                after,
            });
        }
        if edits.is_empty() {
            return;
        }
        let touched: Vec<HexCoord> = edits.iter().map(|e| e.cell).collect();
        // Coalesce a rapid run of same-cell hex format edits (a colour
        // drag) into one undo step, mirroring the square sheet.
        let recent = self.frame_time - self.last_hex_format_time < COALESCE_WINDOW;
        if recent && self.last_hex_format_cells == touched {
            match self.history.pop_undo() {
                Some(Action::HexFormats(prev)) => {
                    let merged = merge_hex_format_edits(prev, edits);
                    self.history.record(Action::HexFormats(merged));
                }
                Some(other) => {
                    // The top wasn't a hex format action — restore it.
                    self.history.record(other);
                    self.history.record(Action::HexFormats(edits));
                }
                None => self.history.record(Action::HexFormats(edits)),
            }
        } else {
            self.history.record(Action::HexFormats(edits));
        }
        self.last_hex_format_time = self.frame_time;
        self.last_hex_format_cells = touched;
    }

    /// Toggle a boolean format flag across the selection — Excel's rule:
    /// if every cell already has the flag, clear it for all; otherwise
    /// set it for all. Works on whichever sheet is active.
    pub(super) fn toggle_range(
        &mut self,
        get: impl Fn(&CellFormat) -> bool,
        set: impl Fn(&mut CellFormat, bool),
    ) {
        let target = match self.active {
            ActiveSheet::Square => {
                let cells = self.square.selection.primary_cells();
                toggle_target(cells.iter().map(|&c| get(&self.square.formats.get(c))))
            }
            ActiveSheet::Hex => {
                let cells = self.hex.selection.primary_cells();
                toggle_target(cells.iter().map(|&c| get(&self.hex.formats.get(c))))
            }
            ActiveSheet::Triangle => {
                let cells = self.triangle.selection.primary_cells();
                toggle_target(cells.iter().map(|&c| get(&self.triangle.formats.get(c))))
            }
            // Voronoi sheet has no format pipeline in v150 — leave the
            // existing toggle target unchanged (defaults to false).
            ActiveSheet::Voronoi => false,
        };
        self.format_range(|f| set(f, target));
    }

    /// Move the selection on whichever sheet is active, collapsing any
    /// square-sheet range to a single cell.
    pub(super) fn move_active(&mut self, dir: Dir) {
        match self.active {
            ActiveSheet::Square => {
                let next = step_square(self.square.selection.cursor, dir);
                self.square.selection.collapse_to(next);
            }
            ActiveSheet::Hex => self.move_hex_selection(dir),
            ActiveSheet::Triangle => self.move_triangle_selection(dir),
            ActiveSheet::Voronoi => self.voronoi_advance(),
        }
    }

    /// Advance the Voronoi cursor to a Delaunay neighbor not in the recent
    /// visit history, picking by closest centroid (lowest-index tie-break).
    /// Updates `voronoi_visit_history` (bounded at `N-1`). No-op when no
    /// candidate qualifies — Enter stops (ADR-014 F3).
    pub(super) fn voronoi_advance(&mut self) {
        let current = self.voronoi.selection.cursor;
        let sid = self.voronoi.sheet_id;
        if let Some(next) =
            voronoi_advance_step(&self.engine, sid, current, &mut self.voronoi_visit_history)
        {
            self.voronoi.selection.collapse_to(next);
        }
    }

    /// Page Up / Page Down — move the cursor a page. The square sheet
    /// shifts by `PAGE_ROWS` rows, clamped; the hex sheet walks to the
    /// r-extreme of the view along the cursor's q-line.
    pub(super) fn page(&mut self, up: bool) {
        match self.active {
            ActiveSheet::Square => {
                let (c, r) = self.square.selection.cursor;
                let row = grid::page_step(r, up, PAGE_ROWS, ROWS - 1);
                self.square.selection.collapse_to((c, row));
            }
            ActiveSheet::Hex => {
                let dir = if up { Dir::Up } else { Dir::Down };
                let next = hex_jump(self.hex.selection.cursor, dir, hex_in_view, |_| false);
                self.hex.selection.collapse_to(next);
            }
            ActiveSheet::Triangle => {
                let cursor = self.triangle.selection.cursor;
                let row = if up {
                    (cursor.row - PAGE_ROWS as i32).max(-TRIANGLE_RADIUS)
                } else {
                    (cursor.row + PAGE_ROWS as i32).min(TRIANGLE_RADIUS)
                };
                self.triangle
                    .selection
                    .collapse_to(TriCoord::new(cursor.col, row));
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// The square-sheet cell a Ctrl+arrow jump lands on from the cursor —
    /// the block edge along `dir`'s row or column, via `grid::jump_target`.
    fn square_jump(&self, dir: Dir) -> (u32, u32) {
        let (c, r) = self.square.selection.cursor;
        match dir {
            Dir::Left => (
                grid::jump_target(c, COLS - 1, false, |i| self.square_occupied(i, r)),
                r,
            ),
            Dir::Right => (
                grid::jump_target(c, COLS - 1, true, |i| self.square_occupied(i, r)),
                r,
            ),
            Dir::Up => (
                c,
                grid::jump_target(r, ROWS - 1, false, |i| self.square_occupied(c, i)),
            ),
            Dir::Down => (
                c,
                grid::jump_target(r, ROWS - 1, true, |i| self.square_occupied(c, i)),
            ),
        }
    }

    /// Jump the cursor to the data edge — Ctrl+arrow, on either sheet.
    pub(super) fn jump_active(&mut self, dir: Dir) {
        match self.active {
            ActiveSheet::Square => {
                let next = self.square_jump(dir);
                self.square.selection.collapse_to(next);
            }
            ActiveSheet::Hex => {
                let next = hex_jump(self.hex.selection.cursor, dir, hex_in_view, |c| {
                    self.hex_occupied(c)
                });
                self.hex.selection.collapse_to(next);
            }
            ActiveSheet::Triangle => {
                // Ctrl+arrow jumps to the data edge — for triangles
                // this lands in a follow-up; for now treat it as a
                // single step like a plain arrow.
                self.move_triangle_selection(dir);
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// Extend the selection to the data edge — Ctrl+Shift+arrow, on
    /// either sheet. Keeps the anchor and jumps the cursor.
    pub(super) fn jump_extend_active(&mut self, dir: Dir) {
        match self.active {
            ActiveSheet::Square => {
                let next = self.square_jump(dir);
                self.square.selection.extend_to(next);
            }
            ActiveSheet::Hex => {
                let next = hex_jump(self.hex.selection.cursor, dir, hex_in_view, |c| {
                    self.hex_occupied(c)
                });
                self.hex.selection.extend_to(next);
            }
            ActiveSheet::Triangle => self.extend_triangle_selection(dir),
            ActiveSheet::Voronoi => {}
        }
    }

    /// Whether a square-sheet cell holds content — the predicate
    /// `grid::jump_target` scans for Ctrl+arrow.
    pub(super) fn square_occupied(&self, col: u32, row: u32) -> bool {
        !matches!(self.cell_value(col, row), CellValue::Empty)
    }

    /// Whether a hex-sheet cell holds content — the predicate the hex
    /// Ctrl+arrow jump scans.
    pub(super) fn hex_occupied(&self, coord: HexCoord) -> bool {
        !matches!(self.hex_cell_value(coord), CellValue::Empty)
    }

    /// Extend the selection one cell — Shift+arrow, on either sheet.
    pub(super) fn extend_active(&mut self, dir: Dir) {
        match self.active {
            ActiveSheet::Square => {
                let next = step_square(self.square.selection.cursor, dir);
                self.square.selection.extend_to(next);
            }
            ActiveSheet::Hex => self.extend_hex_selection(dir),
            ActiveSheet::Triangle => self.extend_triangle_selection(dir),
            ActiveSheet::Voronoi => {}
        }
    }

    /// Step the hex selection one axial cell — collapsing any range and
    /// ignoring a move that would leave the visible disc.
    fn move_hex_selection(&mut self, dir: Dir) {
        let next = hex_step(self.hex.selection.cursor, dir);
        if hex_in_view(next) {
            self.hex.selection.collapse_to(next);
        }
    }

    /// Extend the hex selection one axial cell — Shift+arrow.
    fn extend_hex_selection(&mut self, dir: Dir) {
        let next = hex_step(self.hex.selection.cursor, dir);
        if hex_in_view(next) {
            self.hex.selection.extend_to(next);
        }
    }

    /// Step the triangle selection one cell — Up/Down move along the
    /// `row` axis, Left/Right along `col`. The triangle window is the
    /// `±TRIANGLE_RADIUS` block; moves past the edge are clamped.
    fn move_triangle_selection(&mut self, dir: Dir) {
        let next = triangle_step(self.triangle.selection.cursor, dir);
        if triangle_in_view(next) {
            self.triangle.selection.collapse_to(next);
        }
    }

    /// Extend the triangle selection one cell — Shift+arrow analogue.
    fn extend_triangle_selection(&mut self, dir: Dir) {
        let next = triangle_step(self.triangle.selection.cursor, dir);
        if triangle_in_view(next) {
            self.triangle.selection.extend_to(next);
        }
    }
}
