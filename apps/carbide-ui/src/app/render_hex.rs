//! Hex-lattice rendering, polygon painting, borders, and interaction.

use carbide_core::CellValue;
use carbide_tess::hex::{self, HexCoord};
use carbide_tess::{Lattice, Point2};
use eframe::egui;

use crate::clipboard::{PasteMode, SourceLattice};
use crate::format::{self, VAlign};
use crate::formula_mode;
use crate::widget;

use super::helpers::{
    draw_text_decorations, error_detail, hex_address, hex_in_view, hex_text_layout,
};
use super::types::CellId;
use super::{CarbideApp, HEX_SIZE, HEX_VIEW_RADIUS};

impl CarbideApp {
    /// Paint one hexagon — its polygon and cell value — using vertices
    /// and a centroid straight from the engine's lattice.
    fn paint_hex(
        &self,
        painter: &egui::Painter,
        origin: egui::Pos2,
        coord: HexCoord,
        fill: egui::Color32,
        stroke: egui::Stroke,
        text_color: egui::Color32,
    ) {
        let vertices: Vec<egui::Pos2> = self
            .hex_lattice
            .vertices(coord)
            .iter()
            .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
            .collect();
        let fmt = self.hex_effective_format(coord);
        painter.add(egui::Shape::convex_polygon(vertices.clone(), fill, stroke));
        // Per-edge borders, stroked heavier over the polygon outline.
        if fmt.hex_borders.edges.iter().any(|&on| on) {
            let border = egui::Stroke::new(2.0_f32, text_color);
            for (i, &on) in fmt.hex_borders.edges.iter().enumerate() {
                if on {
                    painter.line_segment([vertices[i], vertices[(i + 1) % 6]], border);
                }
            }
        }

        // Skip the cell-text paint when this cell is a widget — the
        // checkbox / button etc. paints over the centroid, and showing
        // the value text behind it bleeds the source ("TRUE"/"FALSE",
        // the button label) out from under the widget.
        let suppress_text = self.hex_widgets.is_widget(coord);
        let text = if suppress_text {
            String::new()
        } else {
            self.hex_cell_text(coord)
        };
        if !text.is_empty() {
            let value = self.hex_cell_value(coord);
            let numeric = matches!(value, CellValue::Number(_) | CellValue::Integer(_));
            let is_negative = match value {
                CellValue::Number(n) => n < 0.0,
                CellValue::Integer(i) => i < 0,
                _ => false,
            };
            let c = self.hex_lattice.centroid(coord);
            let vshift = match fmt.valign {
                VAlign::Top => -HEX_SIZE * 0.5,
                VAlign::Middle => 0.0,
                VAlign::Bottom => HEX_SIZE * 0.5,
            };
            let centroid = egui::pos2(origin.x + c.x, origin.y + c.y + vshift);
            let align = format::effective_align(fmt.align, numeric);
            let (anchor, pos) = hex_text_layout(align, centroid, HEX_SIZE * 0.6);
            let color = format::effective_text_color(&fmt, is_negative, text_color);
            let font = egui::FontId::proportional(fmt.font_size.points());
            let text_rect = painter.text(pos, anchor, &text, font.clone(), color);
            if fmt.bold {
                // Faux-bold: a second pass nudged half a pixel across.
                painter.text(egui::pos2(pos.x + 0.5, pos.y), anchor, &text, font, color);
            }
            draw_text_decorations(painter, text_rect, &fmt, color);
        }
        // A note marker — a small dot near the hexagon's top.
        if self.hex_notes.has(coord) {
            let centre = self.hex_lattice.centroid(coord);
            let dot = egui::pos2(origin.x + centre.x, origin.y + centre.y - HEX_SIZE * 0.52);
            painter.circle_filled(dot, 3.0, egui::Color32::from_rgb(220, 90, 70));
        }
    }

    pub(super) fn draw_hex_grid(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        // Lattice-space (0,0) is drawn at the panel's centre.
        let origin = response.rect.center();

        // Resolve the cell under each interaction's pointer up front,
        // then release the immutable borrow on `self.hex_lattice` so
        // the handler bodies can call `&mut self` methods (commit_edit,
        // selection mutations).
        let clicked_coord = if response.clicked() {
            response.interact_pointer_pos().and_then(|p| {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                self.hex_lattice.cell_at(local).filter(|c| hex_in_view(*c))
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
                    self.hex_lattice.cell_at(local).filter(|c| hex_in_view(*c))
                })
        } else {
            None
        };
        let dragged_coord = if response.dragged() {
            response.interact_pointer_pos().and_then(|p| {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                self.hex_lattice.cell_at(local).filter(|c| hex_in_view(*c))
            })
        } else {
            None
        };

        if let Some(coord) = clicked_coord {
            // Formula-mode click: insert the hex address into the
            // formula buffer; selection doesn't move.
            let in_formula = self
                .edit
                .as_ref()
                .is_some_and(|e| formula_mode::is_formula_buffer(&e.buffer));
            if in_formula {
                if let Some(edit) = self.edit.as_mut() {
                    let (new_drag, new_hl) = formula_mode::dispatch(
                        &mut edit.buffer,
                        &mut edit.fresh,
                        self.hex.formula_drag,
                        formula_mode::Event::Clicked(coord),
                        hex_address,
                    );
                    self.hex.formula_drag = new_drag;
                    if let Some(h) = new_hl {
                        self.hex.formula_highlight = Some(h);
                    }
                }
            } else {
                self.commit_edit();
                if ui.input(|i| i.modifiers.shift) {
                    self.hex.selection.extend_to(coord);
                } else {
                    self.hex.selection.collapse_to(coord);
                }
            }
        }

        // Drag — either a formula-mode range insert or a sheet
        // selection sweep, matching the square grid's behaviour.
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
                        self.hex.formula_drag,
                        formula_mode::Event::DragStarted(coord),
                        hex_address,
                    );
                    self.hex.formula_drag = new_drag;
                    if let Some(h) = new_hl {
                        self.hex.formula_highlight = Some(h);
                    }
                }
            } else {
                self.commit_edit();
                self.hex.selection.collapse_to(coord);
            }
        }
        if let Some(coord) = dragged_coord {
            if self.hex.formula_drag.is_some() {
                if let Some(edit) = self.edit.as_mut() {
                    let (new_drag, new_hl) = formula_mode::dispatch(
                        &mut edit.buffer,
                        &mut edit.fresh,
                        self.hex.formula_drag,
                        formula_mode::Event::Dragged(coord),
                        hex_address,
                    );
                    self.hex.formula_drag = new_drag;
                    if let Some(h) = new_hl {
                        self.hex.formula_highlight = Some(h);
                    }
                }
            } else {
                self.hex.selection.extend_to(coord);
            }
        }
        if response.drag_stopped() {
            self.hex.formula_drag = None;
        }

        // A double-click on a hex begins editing it in place.
        if response.double_clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                if let Some(coord) = self.hex_lattice.cell_at(local) {
                    if hex_in_view(coord) {
                        self.hex.selection.collapse_to(coord);
                        self.begin_edit(None);
                    }
                }
            }
        }

        // Right-click selects the hex under the pointer (unless it is
        // already selected), then opens the cell-actions menu.
        if response.secondary_clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let local = Point2::new(p.x - origin.x, p.y - origin.y);
                if let Some(coord) = self.hex_lattice.cell_at(local) {
                    if hex_in_view(coord) && !self.hex.selection.contains(coord) {
                        self.commit_edit();
                        self.hex.selection.collapse_to(coord);
                    }
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
                self.open_note(CellId::Hex(self.hex.selection.cursor));
                ui.close_menu();
            }
        });
        // A hovered hex shows its error detail, or failing that its note.
        if let Some(p) = response.hover_pos() {
            let local = Point2::new(p.x - origin.x, p.y - origin.y);
            if let Some(coord) = self.hex_lattice.cell_at(local) {
                if hex_in_view(coord) {
                    if let CellValue::Error(e) = self.hex_cell_value(coord) {
                        let detail = error_detail(&e);
                        egui::show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            egui::Id::new("hex_error_tooltip"),
                            |ui| ui.label(detail),
                        );
                    } else if self.hex_notes.has(coord) {
                        let note = self.hex_notes.get(coord).to_string();
                        egui::show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            egui::Id::new("hex_note_tooltip"),
                            |ui| ui.label(note),
                        );
                    }
                }
            }
        }

        let visuals = ui.visuals();
        let line = egui::Stroke::new(1.0_f32, visuals.weak_text_color());
        let sel_stroke = egui::Stroke::new(2.5_f32, visuals.selection.stroke.color);
        let cell_bg = visuals.extreme_bg_color;
        let sel_bg = visuals.selection.bg_fill;
        let text_color = visuals.text_color();

        let cursor = self.hex.selection.cursor;
        for coord in hex::hex_disc(HexCoord::new(0, 0), HEX_VIEW_RADIUS) {
            if coord == cursor {
                continue;
            }
            // Cells inside the selected range take the selection fill
            // AND the selection's thick stroke, so a multi-cell range
            // reads as one connected highlight instead of a body-only
            // tint with a thick border on the cursor alone.
            let in_selection = self.hex.selection.contains(coord);
            let fill = if in_selection {
                sel_bg
            } else if self.find.is_match(CellId::Hex(coord)) {
                egui::Color32::from_rgb(255, 236, 170)
            } else {
                self.hex_effective_format(coord).fill.unwrap_or(cell_bg)
            };
            let stroke = if in_selection { sel_stroke } else { line };
            self.paint_hex(&painter, origin, coord, fill, stroke, text_color);
        }
        // The cursor hex is painted last so its ring sits above every
        // neighbour's shared border. While editing, the text editor
        // overlay (drawn below) covers the cell's painted value.
        self.paint_hex(&painter, origin, cursor, sel_bg, sel_stroke, text_color);

        // The cut marquee — a dashed outline around each armed hex, when
        // the clipboard's cut belongs to this (hex) sheet.
        if let Some((oq, or)) = self.clipboard.cut_origin() {
            if matches!(self.clipboard.source(), SourceLattice::Hex) {
                let (cw, ch) = self.clipboard.dimensions();
                let dash = egui::Stroke::new(1.5_f32, sel_stroke.color);
                for j in 0..ch {
                    for i in 0..cw {
                        let coord = HexCoord::new(oq + i as i32, or + j as i32);
                        if !hex_in_view(coord) {
                            continue;
                        }
                        let mut loop_pts: Vec<egui::Pos2> = self
                            .hex_lattice
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

        // Formula-reference marquee — outline every hex inside the
        // axial parallelogram of the current formula reference with a
        // dashed blue stroke. Only drawn while an edit is active so it
        // disappears as soon as the formula is committed or cancelled.
        if self.edit.is_some() {
            if let Some(hl) = self.hex.formula_highlight {
                let formula_color = egui::Color32::from_rgb(70, 120, 220);
                let dash = egui::Stroke::new(1.8_f32, formula_color);
                for coord in hex::axial_parallelogram(hl.start, hl.end) {
                    if !hex_in_view(coord) {
                        continue;
                    }
                    let mut loop_pts: Vec<egui::Pos2> = self
                        .hex_lattice
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

        // Hex widgets — render Button + Toggle for cells in
        // `hex_widgets`. Slider / ProgressBar fall through to the
        // ordinary text render (those controls don't fit gracefully
        // inside the 36-point hexagon; revisit if a user asks).
        if !self.hex_widgets.is_empty() {
            let editing_coord = self.edit.as_ref().map(|_| self.hex.selection.cursor);
            let mut hex_edits: Vec<(HexCoord, Option<String>)> = Vec::new();
            for (coord, kind) in self
                .hex_widgets
                .iter()
                .map(|(c, k)| (*c, *k))
                .collect::<Vec<_>>()
            {
                if editing_coord == Some(coord) || !hex_in_view(coord) {
                    continue;
                }
                let centroid = self.hex_lattice.centroid(coord);
                let center = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
                let rect = egui::Rect::from_center_size(
                    center,
                    egui::vec2(HEX_SIZE * 1.4, HEX_SIZE * 0.6),
                );
                let addr = hex_address(coord);
                match kind {
                    widget::WidgetKind::Button => {
                        let label = self
                            .engine
                            .get_cell(self.hex.sheet_id, &addr)
                            .and_then(|s| s.source)
                            .unwrap_or_else(|| "Click".to_string());
                        if ui.put(rect, egui::Button::new(label.clone())).clicked() {
                            // Re-fire the source through the engine — for
                            // `=RANDBETWEEN(...)` this advances the PRNG
                            // and yields a fresh value.
                            hex_edits.push((coord, Some(label)));
                        }
                    }
                    widget::WidgetKind::Toggle => {
                        let value = self
                            .engine
                            .get_cell(self.hex.sheet_id, &addr)
                            .map(|s| s.value)
                            .unwrap_or(CellValue::Empty);
                        let mut checked = widget::bool_state(&value);
                        if ui
                            .put(rect, egui::Checkbox::new(&mut checked, ""))
                            .changed()
                        {
                            hex_edits.push((coord, Some(widget::bool_source(checked).to_string())));
                        }
                    }
                    // Slider / ProgressBar on hex falls through to the
                    // ordinary text render (the cell value still shows).
                    widget::WidgetKind::Slider { .. } | widget::WidgetKind::ProgressBar { .. } => {}
                }
            }
            for (coord, source) in hex_edits {
                let _ =
                    self.engine
                        .set_cell(self.hex.sheet_id, &hex_address(coord), source.as_deref());
            }
        }

        // The in-cell editor overlay, sized to sit within the hexagon.
        if let Some(edit) = &mut self.edit {
            let centroid = self.hex_lattice.centroid(self.hex.selection.cursor);
            let center = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
            let rect = egui::Rect::from_center_size(center, egui::vec2(1.6 * HEX_SIZE, 24.0));
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
    }
}
