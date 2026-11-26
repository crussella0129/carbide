//! Square-grid rendering, viewport culling, fill handle, and header interaction.

use carbide_core::CellValue;
use eframe::egui;

use crate::clipboard::{PasteMode, SourceLattice};
use crate::conditional;
use crate::format::Borders;
use crate::formula_mode;
use crate::grid;
use crate::selection::Selection;
use crate::widget;

use super::helpers::{draw_borders, draw_cell_text, draw_note_marker, error_detail};
use super::types::{CellId, FillAxis, FillDrag, HeaderDrag};
use super::{CarbideApp, COLS, ROWS};

impl CarbideApp {
    pub(super) fn draw_grid(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(
            self.metrics.total_width(COLS),
            self.metrics.total_height(ROWS),
        );
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let origin = response.rect.min;
        // Both headers are frozen: each floats to its viewport edge so
        // the A/B/C strip and the 1/2/3 column stay visible while the
        // cells scroll. The corner box rides them both.
        let header_x = ui.clip_rect().left();
        let header_y = ui.clip_rect().top();
        let col_hdr_origin = egui::pos2(origin.x, header_y);
        let row_hdr_origin = egui::pos2(header_x, origin.y);
        let corner_origin = egui::pos2(header_x, header_y);

        // Capture the press position the moment the button goes down,
        // before egui's drag threshold has had a chance to fire — its
        // ~5-pixel slop is enough to leave a border's grab zone and
        // make a resize attempt read as a header-select drag instead.
        // `interact_pointer_pos` is `Some` while a press is active, so
        // the None → Some transition is the press frame.
        let interact_pos = response.interact_pointer_pos();
        match (self.press_pos, interact_pos) {
            (None, Some(p)) => self.press_pos = Some(p),
            (Some(_), None) => self.press_pos = None,
            _ => {}
        }

        // Keep the active cell in view when the keyboard moves it past a
        // scroll edge. `None` alignment scrolls the minimum amount, so an
        // already-visible cell (e.g. one just clicked) is left untouched.
        if self.square.selection.cursor != self.prev_cursor {
            let (c, r) = self.square.selection.cursor;
            let cursor_rect = egui::Rect::from_min_size(
                origin + egui::vec2(self.metrics.col_left(c), self.metrics.row_top(r)),
                egui::vec2(self.metrics.col_width(c), self.metrics.row_height(r)),
            );
            ui.scroll_to_rect(cursor_rect, None);
            self.prev_cursor = self.square.selection.cursor;
        }

        let fill_handle_visual = self.fill_handle_visual_rect(origin);
        let fill_handle_hit = self.fill_handle_hit_rect(origin);
        if let Some(p) = response.hover_pos() {
            if self
                .metrics
                .col_border_at(col_hdr_origin, p, COLS)
                .is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            } else if self
                .metrics
                .row_border_at(row_hdr_origin, p, ROWS)
                .is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeRow);
            } else if fill_handle_hit.contains(p) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }
        self.handle_resize(&response, col_hdr_origin, row_hdr_origin);

        // A hovered cell shows its error detail, or failing that its note.
        if let Some(p) = response.hover_pos() {
            if let Some(cell) = self
                .metrics
                .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
            {
                if let CellValue::Error(e) = self.cell_value(cell.0, cell.1) {
                    let detail = error_detail(&e);
                    egui::show_tooltip_at_pointer(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("cell_error_tooltip"),
                        |ui| ui.label(detail),
                    );
                } else if self.notes.has(cell) {
                    let note = self.notes.get(cell).to_string();
                    egui::show_tooltip_at_pointer(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("cell_note_tooltip"),
                        |ui| ui.label(note),
                    );
                }
            }
        }

        // Double-clicking a column or row border autofits it to content.
        if response.double_clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                if let Some(col) = self.metrics.col_border_at(col_hdr_origin, p, COLS) {
                    self.autofit_column(col, &painter);
                } else if let Some(row) = self.metrics.row_border_at(row_hdr_origin, p, ROWS) {
                    self.autofit_row(row, &painter);
                }
            }
        }

        // A drag that didn't start on a header border sweeps a selection:
        // a cell drag a cell range, a header drag a column/row range.
        // The drag_started arm hit-tests against `press_pos` for the
        // same reason `handle_resize` does — egui's threshold can leave
        // the pointer a band-width away from where the user actually
        // clicked.
        if self.resizing.is_none() {
            let in_formula = self
                .edit
                .as_ref()
                .is_some_and(|e| formula_mode::is_formula_buffer(&e.buffer));
            if response.drag_started() {
                if let Some(p) = self.press_pos.or_else(|| response.interact_pointer_pos()) {
                    if fill_handle_hit.contains(p) {
                        // Fill-handle drag — record the ORIGINAL bounds
                        // at drag start; subsequent dragged frames
                        // extend the selection (locked to a single
                        // axis chosen from drag direction) and the
                        // release applies the per-lane fill.
                        self.commit_edit();
                        self.fill_drag = Some(FillDrag {
                            original: self.square.selection.bounds(),
                            axis: None,
                        });
                    } else if in_formula {
                        // Formula-mode drag — `formula_mode::drag_start`
                        // writes the start cell's address, records the
                        // anchor, and gives us the initial highlight.
                        if let Some(cell) = self
                            .metrics
                            .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
                        {
                            if let Some(edit) = self.edit.as_mut() {
                                let (new_drag, new_hl) = formula_mode::dispatch(
                                    &mut edit.buffer,
                                    &mut edit.fresh,
                                    self.square.formula_drag,
                                    formula_mode::Event::DragStarted(cell),
                                    |c| grid::cell_address(c.0, c.1),
                                );
                                self.square.formula_drag = new_drag;
                                if let Some(h) = new_hl {
                                    self.square.formula_highlight = Some(h);
                                }
                            }
                        }
                    } else if let Some(col) = self.metrics.col_header_at(col_hdr_origin, p, COLS) {
                        self.commit_edit();
                        self.header_drag = Some(HeaderDrag::Column(col));
                        self.square.selection = Selection::column(col, ROWS);
                    } else if let Some(row) = self.metrics.row_header_at(row_hdr_origin, p, ROWS) {
                        self.commit_edit();
                        self.header_drag = Some(HeaderDrag::Row(row));
                        self.square.selection = Selection::row(row, COLS);
                    } else if let Some(cell) = self
                        .metrics
                        .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
                    {
                        self.commit_edit();
                        self.square.selection.collapse_to(cell);
                    }
                }
            } else if response.dragged() {
                if let Some(p) = response.interact_pointer_pos() {
                    if let Some(mut fill) = self.fill_drag {
                        if let Some(cell) = self
                            .metrics
                            .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
                        {
                            let ((oc0, or0), (oc1, or1)) = fill.original;
                            // Pick the lock axis on the first move that
                            // actually leaves the original bounds, then
                            // keep it stable for the rest of the drag.
                            // Excel widths the handle's catchment but
                            // still locks to one dimension.
                            if fill.axis.is_none() {
                                let outside_below = cell.1 > or1;
                                let outside_right = cell.0 > oc1;
                                if outside_below && !outside_right {
                                    fill.axis = Some(FillAxis::Down);
                                } else if outside_right && !outside_below {
                                    fill.axis = Some(FillAxis::Right);
                                } else if outside_below && outside_right {
                                    // Pick the axis the pointer has
                                    // travelled farther on.
                                    let row_dist = cell.1 - or1;
                                    let col_dist = cell.0 - oc1;
                                    fill.axis = Some(if row_dist >= col_dist {
                                        FillAxis::Down
                                    } else {
                                        FillAxis::Right
                                    });
                                }
                            }
                            // Clamp the cursor to the locked axis so a
                            // diagonal drag still extends only along
                            // that axis.
                            let clamped = match fill.axis {
                                Some(FillAxis::Down) => (cell.0.clamp(oc0, oc1), cell.1.max(or0)),
                                Some(FillAxis::Right) => (cell.0.max(oc0), cell.1.clamp(or0, or1)),
                                None => (cell.0.clamp(oc0, oc1), cell.1.clamp(or0, or1)),
                            };
                            self.square.selection.extend_to(clamped);
                            self.fill_drag = Some(fill);
                        }
                    } else if self.square.formula_drag.is_some() {
                        if let Some(cell) = self
                            .metrics
                            .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
                        {
                            if let Some(edit) = self.edit.as_mut() {
                                let (new_drag, new_hl) = formula_mode::dispatch(
                                    &mut edit.buffer,
                                    &mut edit.fresh,
                                    self.square.formula_drag,
                                    formula_mode::Event::Dragged(cell),
                                    |c| grid::cell_address(c.0, c.1),
                                );
                                self.square.formula_drag = new_drag;
                                if let Some(h) = new_hl {
                                    self.square.formula_highlight = Some(h);
                                }
                            }
                        }
                    } else {
                        match self.header_drag {
                            Some(HeaderDrag::Column(c0)) => {
                                let c1 = self.metrics.col_at_x(origin, p, COLS);
                                self.square.selection = Selection::column_range(c0, c1, ROWS);
                            }
                            Some(HeaderDrag::Row(r0)) => {
                                let r1 = self.metrics.row_at_y(origin, p, ROWS);
                                self.square.selection = Selection::row_range(r0, r1, COLS);
                            }
                            None => {
                                if let Some(cell) = self
                                    .metrics
                                    .cell_at_frozen(origin, header_x, header_y, p, COLS, ROWS)
                                {
                                    self.square.selection.extend_to(cell);
                                }
                            }
                        }
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.header_drag = None;
            self.square.formula_drag = None;
            if let Some(fill) = self.fill_drag.take() {
                // Skip the apply if the user never moved out of the
                // original — a tap-on-handle becomes a no-op (no
                // accidental overwrites).
                let extended = self.square.selection.bounds();
                if extended != fill.original {
                    self.fill_handle_apply(fill.original, extended);
                }
            }
        }
        if response.clicked() {
            if let Some(cell) = self.cell_under(&response, origin, header_x, header_y) {
                // Formula-mode click — while the in-cell edit is a formula
                // (`=…`), a click on another cell inserts that cell's A1
                // address into the formula at the end of the buffer
                // instead of committing the edit and moving the selection.
                // The TextEdit re-focuses next frame via `fresh = true`.
                let in_formula = self
                    .edit
                    .as_ref()
                    .is_some_and(|e| formula_mode::is_formula_buffer(&e.buffer));
                if in_formula {
                    if let Some(edit) = self.edit.as_mut() {
                        let (new_drag, new_hl) = formula_mode::dispatch(
                            &mut edit.buffer,
                            &mut edit.fresh,
                            self.square.formula_drag,
                            formula_mode::Event::Clicked(cell),
                            |c| grid::cell_address(c.0, c.1),
                        );
                        self.square.formula_drag = new_drag;
                        if let Some(h) = new_hl {
                            self.square.formula_highlight = Some(h);
                        }
                    }
                } else if let Some(fmt) = self.format_painter.take() {
                    // Format painter — paint the captured format onto the
                    // target cell and disarm; selection doesn't move.
                    self.commit_edit();
                    self.square.formats.update(cell, |f| *f = fmt);
                } else {
                    self.commit_edit();
                    if ui.input(|i| i.modifiers.shift) {
                        self.square.selection.extend_to(cell);
                    } else {
                        self.square.selection.collapse_to(cell);
                    }
                }
            } else if let Some(p) = response.interact_pointer_pos() {
                // A click on a header selects a whole column or row — with
                // Shift, the range from the current anchor. The header
                // corner selects the sheet.
                let shift = ui.input(|i| i.modifiers.shift);
                if let Some(col) = self.metrics.col_header_at(col_hdr_origin, p, COLS) {
                    self.commit_edit();
                    self.square.selection = if shift {
                        Selection::column_range(self.square.selection.anchor.0, col, ROWS)
                    } else {
                        Selection::column(col, ROWS)
                    };
                } else if let Some(row) = self.metrics.row_header_at(row_hdr_origin, p, ROWS) {
                    self.commit_edit();
                    self.square.selection = if shift {
                        Selection::row_range(self.square.selection.anchor.1, row, COLS)
                    } else {
                        Selection::row(row, COLS)
                    };
                } else if grid::in_header_corner(corner_origin, p) {
                    self.commit_edit();
                    self.square.selection = Selection::all(COLS, ROWS);
                }
            }
        }
        // A double-click on a cell begins editing it in place.
        if response.double_clicked() {
            if let Some(cell) = self.cell_under(&response, origin, header_x, header_y) {
                self.square.selection.collapse_to(cell);
                self.begin_edit(None);
            }
        }
        // Right-click selects the cell under the cursor — unless it is
        // already inside the selection, which a right-click keeps — then
        // opens a context menu of the common cell actions.
        if response.secondary_clicked() {
            if let Some(cell) = self.cell_under(&response, origin, header_x, header_y) {
                if !self.square.selection.contains(cell) {
                    self.commit_edit();
                    self.square.selection.collapse_to(cell);
                }
            }
        }
        response.context_menu(|ui| {
            if ui.button("Copy").clicked() {
                self.copy_selection(ui.ctx());
                ui.close_menu();
            }
            if ui.button("Cut").clicked() {
                self.cut_selection(ui.ctx());
                ui.close_menu();
            }
            if ui.button("Paste").clicked() {
                self.paste(PasteMode::Normal);
                ui.close_menu();
            }
            if ui.button("Paste values").clicked() {
                self.paste(PasteMode::ValuesOnly);
                ui.close_menu();
            }
            if ui.button("Clear").clicked() {
                self.clear_active();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Select region").clicked() {
                self.select_region();
                ui.close_menu();
            }
            if ui.button("Toggle checkbox").clicked() {
                self.toggle_widget_cells();
                ui.close_menu();
            }
            if ui.button("Fill series").clicked() {
                self.fill_series();
                ui.close_menu();
            }
            ui.menu_button("Sort", |ui| {
                if ui.button("Ascending").clicked() {
                    self.sort_selection(true);
                    ui.close_menu();
                }
                if ui.button("Descending").clicked() {
                    self.sort_selection(false);
                    ui.close_menu();
                }
            });
            if ui.button("Edit note…").clicked() {
                self.open_note(CellId::Square(self.square.selection.cursor));
                ui.close_menu();
            }
        });

        let visuals = ui.visuals();
        let grid_line = egui::Stroke::new(1.0_f32, visuals.weak_text_color());
        let header_bg = visuals.faint_bg_color;
        let cell_bg = visuals.extreme_bg_color;
        let text_color = visuals.text_color();
        let sel_color = visuals.selection.stroke.color;
        let sel = visuals.selection.bg_fill;
        let sel_tint = egui::Color32::from_rgba_unmultiplied(sel.r(), sel.g(), sel.b(), 64);
        let find_tint = egui::Color32::from_rgba_unmultiplied(255, 200, 0, 80);
        let border_stroke = egui::Stroke::new(2.0_f32, text_color);
        let font = egui::FontId::proportional(13.0);

        // Viewport culling: only the cells whose span overlaps the
        // scroll clip-rect get painted, instead of all COLS×ROWS = 10,400
        // every frame. The frozen-header strips and the heavy-border /
        // widget passes reuse the same window.
        let clip = ui.clip_rect();
        let (c0, c1) = self
            .metrics
            .visible_col_range(origin.x, clip.left(), clip.right(), COLS);
        let (r0, r1) = self
            .metrics
            .visible_row_range(origin.y, clip.top(), clip.bottom(), ROWS);
        // Prefix-sum layout: O(1) cell positioning for this frame, vs.
        // `cell_rect`'s O(index) walk. Built once; `cell_rect` itself
        // stays for hit-testing / resize callers outside the loop.
        let layout = self.metrics.layout(COLS, ROWS);

        // Cells: fill, the selection tint, border, and the formatted
        // value. The cell being edited is left blank for the overlay.
        let cursor = self.square.selection.cursor;
        let editing_cell = self.edit.as_ref().map(|_| cursor);
        for r in r0..=r1 {
            for c in c0..=c1 {
                let rect = layout.cell_rect(origin, c, r);
                let base = self.square.formats.get((c, r));
                let fmt = if self.cond_rules.is_empty() {
                    base
                } else {
                    let value = self.cell_value(c, r);
                    conditional::effective_format(&base, &value, &self.cond_rules)
                };
                painter.rect_filled(rect, 0.0, fmt.fill.unwrap_or(cell_bg));
                // The active cell stays untinted; the rest of the range
                // gets a translucent wash, the way Excel/Sheets show it.
                if (c, r) != cursor && self.square.selection.contains((c, r)) {
                    painter.rect_filled(rect, 0.0, sel_tint);
                }
                if self.find.is_match(CellId::Square((c, r))) {
                    painter.rect_filled(rect, 0.0, find_tint);
                }
                painter.rect_stroke(rect, 0.0, grid_line);
                if editing_cell == Some((c, r)) {
                    continue;
                }
                // Widget cells (checkbox, slider) draw their own
                // affordance in a later pass and skip the text pass.
                if self.square_widgets.is_widget((c, r)) {
                    continue;
                }
                let text = self.cell_text(c, r);
                if !text.is_empty() {
                    let value = self.cell_value(c, r);
                    let numeric = matches!(value, CellValue::Number(_) | CellValue::Integer(_));
                    let is_negative = match value {
                        CellValue::Number(n) => n < 0.0,
                        CellValue::Integer(i) => i < 0,
                        _ => false,
                    };
                    draw_cell_text(
                        &painter,
                        &text,
                        rect,
                        &fmt,
                        numeric,
                        is_negative,
                        text_color,
                    );
                }
                if self.notes.has((c, r)) {
                    draw_note_marker(&painter, rect);
                }
            }
        }

        // A second pass for the heavy format borders, so a neighbouring
        // cell's grid line does not overpaint a bordered range's right
        // or bottom perimeter.
        for r in r0..=r1 {
            for c in c0..=c1 {
                let base = self.square.formats.get((c, r));
                let fmt = if self.cond_rules.is_empty() {
                    base
                } else {
                    conditional::effective_format(&base, &self.cell_value(c, r), &self.cond_rules)
                };
                if fmt.borders == Borders::default() {
                    continue;
                }
                let rect = layout.cell_rect(origin, c, r);
                draw_borders(&painter, rect, &fmt.borders, border_stroke);
            }
        }

        // The range border, when the selection spans more than one cell.
        if self.square.selection.is_range() {
            let ((min_c, min_r), (max_c, max_r)) = self.square.selection.bounds();
            let tl = self.metrics.cell_rect(origin, min_c, min_r);
            let br = self.metrics.cell_rect(origin, max_c, max_r);
            painter.rect_stroke(
                egui::Rect::from_min_max(tl.min, br.max),
                0.0,
                egui::Stroke::new(1.5_f32, sel_color),
            );
        }
        // The active-cell ring.
        let (sc, sr) = cursor;
        painter.rect_stroke(
            self.metrics.cell_rect(origin, sc, sr),
            0.0,
            egui::Stroke::new(2.0_f32, sel_color),
        );

        // The fill handle — a small filled square at the bottom-right
        // of the selection's bounding rect. The hit-test zone is wider
        // than the drawn square so near-misses still grab.
        painter.rect_filled(fill_handle_visual, 0.0, sel_color);
        painter.rect_stroke(fill_handle_visual, 0.0, egui::Stroke::new(1.0_f32, cell_bg));

        // The cut marquee — a dashed border around the armed range, when
        // the clipboard's cut belongs to this (square) sheet.
        if let Some((oc, or)) = self.clipboard.cut_origin() {
            if matches!(self.clipboard.source(), SourceLattice::Square) {
                let (cw, ch) = self.clipboard.dimensions();
                let tl = self.metrics.cell_rect(origin, oc as u32, or as u32);
                let br = self
                    .metrics
                    .cell_rect(origin, oc as u32 + cw - 1, or as u32 + ch - 1);
                let rect = egui::Rect::from_min_max(tl.min, br.max);
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                    rect.left_top(),
                ];
                painter.extend(egui::Shape::dashed_line(
                    &corners,
                    egui::Stroke::new(1.5_f32, sel_color),
                    4.0,
                    3.0,
                ));
            }
        }

        // The formula-reference marquee — a dashed blue border around
        // the cells the in-progress formula points at, set by a
        // formula-mode click or drag (v106/v107). Only drawn while an
        // edit is active so it disappears as soon as the formula is
        // committed or cancelled.
        if self.edit.is_some() {
            if let Some(hl) = self.square.formula_highlight {
                let (sc, sr) = hl.start;
                let (ec, er) = hl.end;
                let (min_c, max_c) = if sc <= ec { (sc, ec) } else { (ec, sc) };
                let (min_r, max_r) = if sr <= er { (sr, er) } else { (er, sr) };
                let tl = self.metrics.cell_rect(origin, min_c, min_r);
                let br = self.metrics.cell_rect(origin, max_c, max_r);
                let rect = egui::Rect::from_min_max(tl.min, br.max);
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                    rect.left_top(),
                ];
                let formula_color = egui::Color32::from_rgb(70, 120, 220);
                painter.extend(egui::Shape::dashed_line(
                    &corners,
                    egui::Stroke::new(1.8_f32, formula_color),
                    5.0,
                    3.0,
                ));
            }
        }

        // Widget cells render with their kind's affordance: a
        // clickable checkbox for toggles, a draggable slider for
        // sliders. Each kind produces a (addr, source) write when the
        // user interacts; the writes apply as one edit per frame to
        // keep the engine recompute count bounded.
        if !self.square_widgets.is_empty() {
            let mut edits: Vec<(String, Option<String>)> = Vec::new();
            for r in r0..=r1 {
                for c in c0..=c1 {
                    if editing_cell == Some((c, r)) {
                        continue;
                    }
                    let Some(kind) = self.square_widgets.kind((c, r)) else {
                        continue;
                    };
                    let rect = layout.cell_rect(origin, c, r);
                    match kind {
                        widget::WidgetKind::Toggle => {
                            let mut checked = widget::bool_state(&self.cell_value(c, r));
                            if ui
                                .put(rect, egui::Checkbox::new(&mut checked, ""))
                                .changed()
                            {
                                edits.push((
                                    grid::cell_address(c, r),
                                    Some(widget::bool_source(checked).to_string()),
                                ));
                            }
                        }
                        widget::WidgetKind::Slider { min, max } => {
                            let mut value = widget::slider_value(&self.cell_value(c, r), min, max);
                            // Drag-only slider (no numeric editor) — the
                            // cell stays terse and looks like an app
                            // control rather than a spreadsheet cell.
                            let response = ui.put(
                                rect.shrink(2.0),
                                egui::Slider::new(&mut value, min..=max).show_value(true),
                            );
                            if response.changed() {
                                edits.push((
                                    grid::cell_address(c, r),
                                    Some(widget::slider_source(value)),
                                ));
                            }
                        }
                        widget::WidgetKind::Button => {
                            // Cell source is the label; an empty cell
                            // gets a placeholder so the button is
                            // still clickable.
                            let label = self.cell_source(c, r);
                            let label = if label.is_empty() {
                                "Click".to_string()
                            } else {
                                label
                            };
                            if ui
                                .put(rect.shrink(2.0), egui::Button::new(label.clone()))
                                .clicked()
                            {
                                // Re-fire the cell's own source through
                                // the engine. A side-effecting expression
                                // (RAND, NOW…) recomputes; a literal
                                // round-trips to itself.
                                edits.push((grid::cell_address(c, r), Some(label)));
                            }
                        }
                        widget::WidgetKind::ProgressBar { max } => {
                            // Read-only display — the engine drives
                            // the cell's value, the bar shows
                            // progress, no write-back ever happens.
                            let fraction = widget::progress_fraction(&self.cell_value(c, r), max);
                            ui.put(
                                rect.shrink(2.0),
                                egui::ProgressBar::new(fraction).show_percentage(),
                            );
                        }
                    }
                }
            }
            if !edits.is_empty() {
                self.apply_edits(self.square.sheet_id, edits);
            }
        }

        // The in-cell editor overlay.
        if let Some(edit) = &mut self.edit {
            let rect = self.metrics.cell_rect(origin, sc, sr);
            let response = ui.put(
                rect,
                egui::TextEdit::singleline(&mut edit.buffer)
                    .frame(true)
                    .font(egui::TextStyle::Monospace),
            );
            if std::mem::take(&mut edit.fresh) {
                response.request_focus();
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                    let end = egui::text::CCursor::new(edit.buffer.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(end)));
                    egui::TextEdit::store_state(ui.ctx(), response.id, state);
                }
            }
        }

        // The frozen row + column headers (and their shared corner) —
        // floated to the viewport's edges so the 1/2/3 strip and A/B/C
        // strip stay visible as the cells scroll. Skipped entirely in
        // Stage Mode so the sheet reads as an app, not a spreadsheet.
        if !self.stage_mode {
            // Row header — drawn after the cells so it overpaints any
            // column that scrolls beneath it. Culled to the visible row
            // window, same as the cell passes.
            for r in r0..=r1 {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(header_x, origin.y + layout.row_top(r)),
                    egui::vec2(grid::HEADER_W, layout.row_height(r)),
                );
                painter.rect_filled(rect, 0.0, header_bg);
                // Tint the active cell's row header.
                if r == self.square.selection.cursor.1 {
                    painter.rect_filled(rect, 0.0, sel_tint);
                }
                painter.rect_stroke(rect, 0.0, grid_line);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    (r + 1).to_string(),
                    font.clone(),
                    text_color,
                );
            }
            // Column header.
            for c in c0..=c1 {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(origin.x + layout.col_left(c), header_y),
                    egui::vec2(layout.col_width(c), grid::HEADER_H),
                );
                painter.rect_filled(rect, 0.0, header_bg);
                // Tint the active cell's column header.
                if c == self.square.selection.cursor.0 {
                    painter.rect_filled(rect, 0.0, sel_tint);
                }
                painter.rect_stroke(rect, 0.0, grid_line);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    grid::column_label(c),
                    font.clone(),
                    text_color,
                );
            }
            // The header corner — drawn last so it always sits above
            // both frozen header bands.
            let corner_rect = egui::Rect::from_min_size(
                corner_origin,
                egui::vec2(grid::HEADER_W, grid::HEADER_H),
            );
            painter.rect_filled(corner_rect, 0.0, header_bg);
            painter.rect_stroke(corner_rect, 0.0, grid_line);
        }
    }
}
