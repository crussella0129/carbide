//! Triangle-lattice rendering and interaction.

use carbide_core::CellValue;
use carbide_tess::triangle::TriCoord;
use carbide_tess::{Lattice, Point2};
use eframe::egui;

use crate::clipboard::SourceLattice;
use crate::formula_mode;
use crate::selection::Coord;
use crate::widget;

use super::helpers::{blend_over, triangle_address, triangle_in_view};
use super::{CarbideApp, TRIANGLE_RADIUS, TRIANGLE_SIDE};

impl CarbideApp {
    /// Render the triangle sheet — fill, outline, and text per cell;
    /// click-to-select; double-click to edit. Mirrors `draw_hex_grid`'s
    /// shape but minus the formula-marquee, fill drag, and disc clip.
    /// Triangles outside `triangle_in_view` aren't drawn.
    pub(super) fn draw_triangle_grid(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let origin = response.rect.center();

        // Pre-resolve the cells under each pointer interaction before
        // touching `&mut self`, same shape as draw_hex_grid.
        let clicked_coord = if response.clicked() {
            response.interact_pointer_pos().and_then(|p| {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                self.triangle_lattice
                    .cell_at(local)
                    .filter(|c| triangle_in_view(*c))
            })
        } else {
            None
        };
        let drag_started_coord = if response.drag_started() {
            response
                .interact_pointer_pos()
                .or(self.press_pos)
                .and_then(|p| {
                    let local = Point2::new(p.x - origin.x, p.y - origin.y);
                    self.triangle_lattice
                        .cell_at(local)
                        .filter(|c| triangle_in_view(*c))
                })
        } else {
            None
        };
        let dragged_coord = if response.dragged() {
            response.interact_pointer_pos().and_then(|p| {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                self.triangle_lattice
                    .cell_at(local)
                    .filter(|c| triangle_in_view(*c))
            })
        } else {
            None
        };

        if let Some(coord) = clicked_coord {
            // Formula-mode click: insert the triangle address into
            // the formula buffer; selection doesn't move.
            let in_formula = self
                .edit
                .as_ref()
                .is_some_and(|e| formula_mode::is_formula_buffer(&e.buffer));
            if in_formula {
                if let Some(edit) = self.edit.as_mut() {
                    let (new_drag, new_hl) = formula_mode::dispatch(
                        &mut edit.buffer,
                        &mut edit.fresh,
                        self.triangle.formula_drag,
                        formula_mode::Event::Clicked(coord),
                        triangle_address,
                    );
                    self.triangle.formula_drag = new_drag;
                    if let Some(h) = new_hl {
                        self.triangle.formula_highlight = Some(h);
                    }
                }
            } else {
                self.commit_edit();
                if ui.input(|i| i.modifiers.shift) {
                    self.triangle.selection.extend_to(coord);
                } else {
                    self.triangle.selection.collapse_to(coord);
                }
            }
        }
        if let Some(coord) = drag_started_coord {
            let in_formula = self
                .edit
                .as_ref()
                .is_some_and(|e| formula_mode::is_formula_buffer(&e.buffer));
            if in_formula {
                if let Some(edit) = self.edit.as_mut() {
                    let (new_drag, new_hl) = formula_mode::dispatch(
                        &mut edit.buffer,
                        &mut edit.fresh,
                        self.triangle.formula_drag,
                        formula_mode::Event::DragStarted(coord),
                        triangle_address,
                    );
                    self.triangle.formula_drag = new_drag;
                    if let Some(h) = new_hl {
                        self.triangle.formula_highlight = Some(h);
                    }
                }
            } else {
                self.commit_edit();
                self.triangle.selection.collapse_to(coord);
            }
        }
        if let Some(coord) = dragged_coord {
            if self.triangle.formula_drag.is_some() {
                if let Some(edit) = self.edit.as_mut() {
                    let (new_drag, new_hl) = formula_mode::dispatch(
                        &mut edit.buffer,
                        &mut edit.fresh,
                        self.triangle.formula_drag,
                        formula_mode::Event::Dragged(coord),
                        triangle_address,
                    );
                    self.triangle.formula_drag = new_drag;
                    if let Some(h) = new_hl {
                        self.triangle.formula_highlight = Some(h);
                    }
                }
            } else {
                self.triangle.selection.extend_to(coord);
            }
        }
        if response.drag_stopped() {
            self.triangle.formula_drag = None;
        }
        if response.double_clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                if let Some(coord) = self.triangle_lattice.cell_at(local) {
                    if triangle_in_view(coord) {
                        self.triangle.selection.collapse_to(coord);
                        self.begin_edit(None);
                    }
                }
            }
        }
        if response.secondary_clicked() {
            // Right-click clears any in-flight edit (triangle context
            // menu lands in a follow-up).
            self.commit_edit();
        }

        // First pass: fills (so outlines and text draw on top).
        let edge = ui.visuals().widgets.noninteractive.fg_stroke.color;
        let outline = egui::Stroke::new(1.0_f32, edge);
        let selected_color = egui::Color32::from_rgba_unmultiplied(70, 120, 220, 60);
        let r = TRIANGLE_RADIUS;
        for row in -r..=r {
            for col in -r..=r {
                let coord = TriCoord::new(col, row);
                let verts: Vec<egui::Pos2> = self
                    .triangle_lattice
                    .vertices(coord)
                    .iter()
                    .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                    .collect();
                if verts.len() != 3 {
                    continue;
                }
                let format = self.triangle_effective_format(coord);
                let mut fill = format.fill.unwrap_or(egui::Color32::TRANSPARENT);
                if self.triangle.selection.contains(coord) {
                    // Tint the fill with the selection colour so the
                    // active range stands out even when the cell has
                    // its own fill colour.
                    fill = blend_over(selected_color, fill);
                }
                if fill != egui::Color32::TRANSPARENT {
                    painter.add(egui::Shape::convex_polygon(
                        verts.clone(),
                        fill,
                        egui::Stroke::NONE,
                    ));
                }
                // Outline.
                let mut loop_pts = verts.clone();
                loop_pts.push(verts[0]);
                painter.add(egui::Shape::line(loop_pts, outline));
            }
        }

        // Second pass: cell text at each centroid.
        let fg = ui.visuals().text_color();
        for row in -r..=r {
            for col in -r..=r {
                let coord = TriCoord::new(col, row);
                // Skip widget cells — the Button/Toggle render paints
                // over the centroid; showing the source text behind it
                // bleeds "TRUE"/"FALSE" / the button label through.
                if self.triangle_widgets.is_widget(coord) {
                    continue;
                }
                let centroid = self.triangle_lattice.centroid(coord);
                let p = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
                let text = self.triangle_cell_text(coord);
                if text.is_empty() {
                    continue;
                }
                painter.text(
                    p,
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(13.0),
                    fg,
                );
            }
        }

        // Selection outline — every selected cell gets a brighter edge.
        let selection_stroke = egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(70, 120, 220));
        for coord in self.triangle.selection.cells() {
            if !triangle_in_view(coord) {
                continue;
            }
            let verts: Vec<egui::Pos2> = self
                .triangle_lattice
                .vertices(coord)
                .iter()
                .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                .collect();
            if verts.len() != 3 {
                continue;
            }
            let mut loop_pts = verts.clone();
            loop_pts.push(verts[0]);
            painter.add(egui::Shape::line(loop_pts, selection_stroke));
        }

        // Cut marquee — a dashed outline around each armed triangle,
        // when the clipboard's cut belongs to this (triangle) sheet.
        if let Some((oc, or)) = self.clipboard.cut_origin() {
            if matches!(self.clipboard.source(), SourceLattice::Triangle) {
                let (cw, ch) = self.clipboard.dimensions();
                let dash = egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(70, 120, 220));
                for j in 0..ch {
                    for i in 0..cw {
                        let coord = TriCoord::new(oc + i as i32, or + j as i32);
                        if !triangle_in_view(coord) {
                            continue;
                        }
                        let mut loop_pts: Vec<egui::Pos2> = self
                            .triangle_lattice
                            .vertices(coord)
                            .iter()
                            .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                            .collect();
                        if let Some(&first) = loop_pts.first() {
                            loop_pts.push(first);
                        }
                        painter.extend(egui::Shape::dashed_line(&loop_pts, dash, 4.0, 3.0));
                    }
                }
            }
        }

        // Formula-reference marquee — outline every triangle inside the
        // rectangular range of the current formula reference with a
        // dashed blue stroke. Only drawn while an edit is active so it
        // disappears as soon as the formula is committed or cancelled.
        if self.edit.is_some() {
            if let Some(hl) = self.triangle.formula_highlight {
                let formula_color = egui::Color32::from_rgb(70, 120, 220);
                let dash = egui::Stroke::new(1.8_f32, formula_color);
                let (min, max) = hl.start.min_max(hl.end);
                for row in min.row..=max.row {
                    for col in min.col..=max.col {
                        let coord = TriCoord::new(col, row);
                        if !triangle_in_view(coord) {
                            continue;
                        }
                        let mut loop_pts: Vec<egui::Pos2> = self
                            .triangle_lattice
                            .vertices(coord)
                            .iter()
                            .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                            .collect();
                        if let Some(&first) = loop_pts.first() {
                            loop_pts.push(first);
                        }
                        painter.extend(egui::Shape::dashed_line(&loop_pts, dash, 5.0, 3.0));
                    }
                }
            }
        }

        // Triangle widgets — Button + Toggle for cells in
        // `triangle_widgets`. Slider/ProgressBar fall through to the
        // ordinary text render (the inscribed rect of a 56-side
        // equilateral triangle is too small for those controls; same
        // deferral as hex per ADR-006).
        if !self.triangle_widgets.is_empty() {
            let editing_coord = self.edit.as_ref().map(|_| self.triangle.selection.cursor);
            let mut tri_edits: Vec<(TriCoord, Option<String>)> = Vec::new();
            for (coord, kind) in self
                .triangle_widgets
                .iter()
                .map(|(c, k)| (*c, *k))
                .collect::<Vec<_>>()
            {
                if editing_coord == Some(coord) || !triangle_in_view(coord) {
                    continue;
                }
                let centroid = self.triangle_lattice.centroid(coord);
                let center = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
                let rect = egui::Rect::from_center_size(
                    center,
                    egui::vec2(TRIANGLE_SIDE * 0.7, TRIANGLE_SIDE * 0.4),
                );
                let addr = triangle_address(coord);
                match kind {
                    widget::WidgetKind::Button => {
                        let label = self
                            .engine
                            .get_cell(self.triangle.sheet_id, &addr)
                            .and_then(|s| s.source)
                            .unwrap_or_else(|| "Click".to_string());
                        if ui.put(rect, egui::Button::new(label.clone())).clicked() {
                            tri_edits.push((coord, Some(label)));
                        }
                    }
                    widget::WidgetKind::Toggle => {
                        let value = self
                            .engine
                            .get_cell(self.triangle.sheet_id, &addr)
                            .map(|s| s.value)
                            .unwrap_or(CellValue::Empty);
                        let mut checked = widget::bool_state(&value);
                        if ui
                            .put(rect, egui::Checkbox::new(&mut checked, ""))
                            .changed()
                        {
                            tri_edits.push((coord, Some(widget::bool_source(checked).to_string())));
                        }
                    }
                    widget::WidgetKind::Slider { .. } | widget::WidgetKind::ProgressBar { .. } => {}
                }
            }
            for (coord, source) in tri_edits {
                let _ = self.engine.set_cell(
                    self.triangle.sheet_id,
                    &triangle_address(coord),
                    source.as_deref(),
                );
            }
        }

        // In-cell edit overlay at the cursor centroid.
        if let Some(edit) = &mut self.edit {
            let centroid = self
                .triangle_lattice
                .centroid(self.triangle.selection.cursor);
            let center = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
            let rect = egui::Rect::from_center_size(center, egui::vec2(1.4 * TRIANGLE_SIDE, 24.0));
            let response = ui.put(
                rect,
                egui::TextEdit::singleline(&mut edit.buffer).font(egui::TextStyle::Monospace),
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
    }
}
