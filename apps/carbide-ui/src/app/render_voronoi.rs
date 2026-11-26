//! Voronoi-lattice rendering, seed-handle drag/fade, widget pop-out, and marquee selection.

use carbide_core::CellValue;
use carbide_tess::voronoi::VoronoiCoord;
use carbide_tess::{Lattice, Point2};
use eframe::egui;

use crate::clipboard::PasteMode;
use crate::format;
use crate::formula_mode;
use crate::widget;

use super::helpers::{draw_text_decorations, error_detail};
use super::types::{CellId, EditState};
use super::voronoi_helpers::{
    apply_seed_drag, cells_in_screen_rect, fade_alpha, pop_out_widget_center, seed_handle_alphas,
    synced_voronoi_lattice, voronoi_address, WIDGET_POP_OUT_OFFSET,
};
use super::CarbideApp;

impl CarbideApp {
    /// Render the Voronoi sheet — Demo C from the launch brief. Eight
    /// (or however many seeds the lattice carries) cell polygons fill
    /// the central panel; each cell displays its evaluated value. Click
    /// to select; F2 or double-click to edit. v150 ships read-only-ish
    /// interaction (single-cell selection + formula-bar edit); widgets,
    /// formatting, range selection, fill drag, and copy/paste on
    /// Voronoi all wait for v151.
    pub(super) fn draw_voronoi_grid(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let origin = response.rect.center();

        // Resolve the cell under the pointer up front so the handler bodies
        // can call `&mut self` methods without aliasing.
        let cell_under = response.interact_pointer_pos().and_then(|p| {
            let local = Point2::new(p.x - origin.x, p.y - origin.y);
            self.voronoi_lattice.cell_at(local)
        });
        let clicked_coord = response.clicked().then_some(cell_under).flatten();
        let doubled = response.double_clicked();

        // Formula-mode dispatch — when the edit buffer starts with `=`, route
        // click/drag through `formula_mode::dispatch` so a click inserts the
        // cell's `V(n)` address and a drag extends a `V(a):V(b)` range, just
        // like square/hex/triangle (ADR-013).
        let in_formula = self
            .edit
            .as_ref()
            .is_some_and(|e| formula_mode::is_formula_buffer(&e.buffer));
        let formula_event = if in_formula {
            formula_mode::event_from_response(
                response.clicked(),
                response.drag_started(),
                response.dragged(),
                response.drag_stopped(),
                cell_under,
            )
        } else {
            formula_mode::Event::Idle
        };
        let formula_consumed_click = in_formula
            && matches!(
                formula_event,
                formula_mode::Event::Clicked(_)
                    | formula_mode::Event::DragStarted(_)
                    | formula_mode::Event::Dragged(_)
            );
        if in_formula {
            if let Some(edit) = self.edit.as_mut() {
                let (new_drag, new_hl) = formula_mode::dispatch(
                    &mut edit.buffer,
                    &mut edit.fresh,
                    self.voronoi.formula_drag,
                    formula_event,
                    voronoi_address,
                );
                self.voronoi.formula_drag = new_drag;
                if let Some(h) = new_hl {
                    self.voronoi.formula_highlight = Some(h);
                }
            }
        }

        // Marquee drag (T-006A/B) — when NOT in formula mode, a main-response
        // drag selects every cell whose centroid falls inside the screen
        // rect from press to current pointer (ADR-013 centroid-in-rect rule).
        // **Seed-handle precedence (T-006B):** a press within `SEED_HANDLE_RADIUS`
        // of any seed centroid is treated as a seed-handle drag (handled later
        // in the seed-handle pass) and skips the marquee.
        const SEED_HANDLE_RADIUS: f32 = 5.0;
        if !in_formula {
            if response.drag_started() {
                if let Some(p) = response.interact_pointer_pos() {
                    let on_handle = (0..self.voronoi_lattice.len() as u32).any(|i| {
                        let c = self.voronoi_lattice.centroid(VoronoiCoord(i));
                        let center = egui::pos2(origin.x + c.x, origin.y + c.y);
                        center.distance(p) <= SEED_HANDLE_RADIUS
                    });
                    if !on_handle {
                        self.voronoi_marquee_start = Some(p);
                    }
                }
            }
            if response.dragged() {
                if let (Some(start), Some(current)) =
                    (self.voronoi_marquee_start, response.interact_pointer_pos())
                {
                    let rect = egui::Rect::from_two_pos(start, current);
                    let hits = cells_in_screen_rect(&self.voronoi_lattice, rect, origin);
                    self.voronoi.selection.extra.clear();
                    self.voronoi.selection.extra.extend(hits);
                }
            }
            if response.drag_stopped() {
                // Marquee rect disappears; `selection.extra` persists until
                // the next `collapse_to` (subsequent single click / arrow).
                self.voronoi_marquee_start = None;
            }
        }

        // A click that lands on a widget cell is handled by the widget
        // pass (it owns the checkbox / button hit area), so suppress the
        // select-or-edit behaviour there. A formula-mode click is consumed
        // above and must NOT also collapse the selection.
        let clicked_is_widget = clicked_coord.is_some_and(|c| self.voronoi_widgets.is_widget(c));
        if let Some(coord) = clicked_coord {
            if !clicked_is_widget && !formula_consumed_click {
                self.commit_edit();
                self.voronoi.selection.collapse_to(coord);
                // T-010 (C-009): a primary cell-select click restarts the
                // Enter-advance traversal — clear the visit history so a
                // fresh Enter sequence can visit cells from here.
                self.voronoi_visit_history.clear();
                if doubled {
                    let buffer = self.voronoi_cell_source(coord);
                    self.edit = Some(EditState {
                        buffer,
                        fresh: true,
                    });
                }
            }
        }

        // Right-click selects the Voronoi cell under the pointer (unless it
        // is already part of the selection), then opens the cell-actions menu.
        if response.secondary_clicked() {
            if let Some(coord) = cell_under {
                if !self.voronoi.selection.contains(coord) {
                    self.commit_edit();
                    self.voronoi.selection.collapse_to(coord);
                    self.voronoi_visit_history.clear();
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
            if ui.button("Edit note…").clicked() {
                self.open_note(CellId::Voronoi(self.voronoi.selection.cursor));
                ui.close_menu();
            }
        });

        // A hovered Voronoi cell shows its error detail, or failing that its note.
        if let Some(p) = response.hover_pos() {
            let local = Point2::new(p.x - origin.x, p.y - origin.y);
            if let Some(coord) = self.voronoi_lattice.cell_at(local) {
                if let CellValue::Error(e) = self.voronoi_cell_value(coord) {
                    let detail = error_detail(&e);
                    egui::show_tooltip_at_pointer(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("voronoi_error_tooltip"),
                        |ui| ui.label(detail),
                    );
                } else if self.voronoi_notes.has(coord) {
                    let note = self.voronoi_notes.get(coord).to_string();
                    egui::show_tooltip_at_pointer(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("voronoi_note_tooltip"),
                        |ui| ui.label(note),
                    );
                }
            }
        }

        // First pass — fill + outline every Voronoi cell.
        let stroke =
            egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color);
        let palette: [egui::Color32; 8] = [
            egui::Color32::from_rgb(232, 213, 196),
            egui::Color32::from_rgb(196, 222, 232),
            egui::Color32::from_rgb(207, 232, 196),
            egui::Color32::from_rgb(232, 196, 213),
            egui::Color32::from_rgb(213, 196, 232),
            egui::Color32::from_rgb(232, 230, 196),
            egui::Color32::from_rgb(196, 232, 222),
            egui::Color32::from_rgb(220, 220, 230),
        ];
        for i in 0..self.voronoi_lattice.len() as u32 {
            let coord = VoronoiCoord(i);
            let verts = self.voronoi_lattice.vertices(coord);
            if verts.is_empty() {
                continue;
            }
            let pts: Vec<egui::Pos2> = verts
                .iter()
                .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                .collect();
            let fmt = self.voronoi_effective_format(coord);
            let fill = fmt.fill.unwrap_or(palette[(i as usize) % palette.len()]);
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }

        // Second pass — cell value text at each centroid, formatted with
        // LayoutJob so italics, faux-bold, font size, colour, negative-red,
        // and underline/strikethrough all render faithfully. Widget cells
        // are skipped: the Button/Toggle paints over the centroid, and
        // showing the value behind it bleeds the source through.
        let fg = ui.visuals().text_color();
        for i in 0..self.voronoi_lattice.len() as u32 {
            let coord = VoronoiCoord(i);
            if self.voronoi_widgets.is_widget(coord) {
                continue;
            }
            let centroid = self.voronoi_lattice.centroid(coord);
            let pos = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
            let text = self.voronoi_cell_text(coord);
            if text.is_empty() {
                continue;
            }
            let fmt = self.voronoi_effective_format(coord);
            let value = self.voronoi_cell_value(coord);
            let is_negative = match value {
                CellValue::Number(n) => n < 0.0,
                CellValue::Integer(n) => n < 0,
                _ => false,
            };
            let color = format::effective_text_color(&fmt, is_negative, fg);
            let mut job = egui::text::LayoutJob::default();
            job.append(
                &text,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(fmt.font_size.points()),
                    color,
                    italics: fmt.italic,
                    ..Default::default()
                },
            );
            let galley = painter.layout_job(job);
            let size = galley.size();
            let top_left = egui::pos2(pos.x - size.x / 2.0, pos.y - size.y / 2.0);
            painter.galley(top_left, galley.clone(), color);
            if fmt.bold {
                painter.galley(top_left + egui::vec2(0.55, 0.0), galley, color);
            }
            draw_text_decorations(
                &painter,
                egui::Rect::from_min_size(top_left, size),
                &fmt,
                color,
            );
        }

        // Note marker dots — drawn outside the widget/empty-text early-continue
        // guards so empty or widget cells with notes still display their marker.
        for i in 0..self.voronoi_lattice.len() as u32 {
            let coord = VoronoiCoord(i);
            if self.voronoi_notes.has(coord) {
                let centroid = self.voronoi_lattice.centroid(coord);
                let dot = egui::pos2(origin.x + centroid.x, origin.y + centroid.y - 16.0);
                painter.circle_filled(dot, 3.0, egui::Color32::from_rgb(220, 90, 70));
            }
        }

        // Third pass — selection stroke around every selected cell. Uses
        // `selection.cells()` which returns the anchor + `extra`, so
        // marquee-selected cells get outlined too (ADR-013).
        let sel_stroke = egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(70, 120, 220));
        for coord in self.voronoi.selection.cells() {
            let verts = self.voronoi_lattice.vertices(coord);
            if verts.is_empty() {
                continue;
            }
            let mut pts: Vec<egui::Pos2> = verts
                .iter()
                .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                .collect();
            if let Some(&first) = pts.first() {
                pts.push(first);
            }
            painter.add(egui::Shape::line(pts, sel_stroke));
        }

        // Cut marquee — dashed outline around every Voronoi cell armed for
        // clearing on the next paste (cleared on paste or Escape).
        if let Some(cut_cells) = self.clipboard.cut_voronoi_cells() {
            let dash = egui::Stroke::new(1.5_f32, sel_stroke.color);
            for &coord in cut_cells {
                let verts = self.voronoi_lattice.vertices(coord);
                if verts.is_empty() {
                    continue;
                }
                let mut loop_pts: Vec<egui::Pos2> = verts
                    .iter()
                    .map(|v| egui::pos2(origin.x + v.x, origin.y + v.y))
                    .collect();
                if let Some(&first) = loop_pts.first() {
                    loop_pts.push(first);
                }
                painter.extend(egui::Shape::dashed_line(&loop_pts, dash, 4.0, 3.0));
            }
        }

        // Translucent marquee rect (T-006B) — visible while a marquee drag is
        // in progress; disappears on release (selection.extra persists).
        if let (Some(start), Some(current)) =
            (self.voronoi_marquee_start, response.interact_pointer_pos())
        {
            if response.dragged() {
                let rect = egui::Rect::from_two_pos(start, current);
                painter.rect_filled(
                    rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(70, 120, 220, 30),
                );
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(70, 120, 220)),
                );
            }
        }

        // Widget pass — Button + Toggle inscribed at each widget cell's
        // centroid. Slider/ProgressBar fall through to text (ADR-006).
        // **T-004 (ADR-014):** when the seed handle is visible at this seed
        // (cursor near), the widget pops out to a side so the click area
        // isn't occluded by the handle.
        if !self.voronoi_widgets.is_empty() {
            let editing_coord = self.edit.as_ref().map(|_| self.voronoi.selection.cursor);
            let widget_cursor = response.hover_pos();
            let mut vor_edits: Vec<(VoronoiCoord, Option<String>)> = Vec::new();
            for (coord, kind) in self
                .voronoi_widgets
                .iter()
                .map(|(c, k)| (*c, *k))
                .collect::<Vec<_>>()
            {
                if editing_coord == Some(coord) {
                    continue;
                }
                let centroid = self.voronoi_lattice.centroid(coord);
                // Decide pop-out: handle is "visible" when cursor proximity
                // produces a non-zero fade alpha at this seed AND the sheet
                // isn't frozen (T-008: frozen → no handles → no pop-out).
                let centroid_screen = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
                let frozen = self.engine.voronoi_frozen(self.voronoi.sheet_id);
                let handle_visible = !frozen
                    && match widget_cursor {
                        Some(c) => fade_alpha((centroid_screen - c).length()) > 0,
                        None => false,
                    };
                let lattice_center = if handle_visible {
                    pop_out_widget_center(centroid, WIDGET_POP_OUT_OFFSET, coord, |p| {
                        self.voronoi_lattice.cell_at(p)
                    })
                } else {
                    centroid
                };
                let center = egui::pos2(origin.x + lattice_center.x, origin.y + lattice_center.y);
                let rect = egui::Rect::from_center_size(center, egui::vec2(120.0, 24.0));
                let addr = voronoi_address(coord);
                match kind {
                    widget::WidgetKind::Button => {
                        let label = self
                            .engine
                            .get_cell(self.voronoi.sheet_id, &addr)
                            .and_then(|s| s.source)
                            .unwrap_or_else(|| "Click".to_string());
                        if ui.put(rect, egui::Button::new(label.clone())).clicked() {
                            vor_edits.push((coord, Some(label)));
                        }
                    }
                    widget::WidgetKind::Toggle => {
                        let value = self
                            .engine
                            .get_cell(self.voronoi.sheet_id, &addr)
                            .map(|s| s.value)
                            .unwrap_or(CellValue::Empty);
                        let mut checked = widget::bool_state(&value);
                        if ui
                            .put(rect, egui::Checkbox::new(&mut checked, ""))
                            .changed()
                        {
                            vor_edits.push((coord, Some(widget::bool_source(checked).to_string())));
                        }
                    }
                    widget::WidgetKind::Slider { .. } | widget::WidgetKind::ProgressBar { .. } => {}
                }
            }
            for (coord, source) in vor_edits {
                let _ = self.engine.set_cell(
                    self.voronoi.sheet_id,
                    &voronoi_address(coord),
                    source.as_deref(),
                );
            }
        }

        // Seed-handle pass — a draggable dot at each Voronoi seed. Dragging a
        // handle moves that seed; the engine rebuilds the lattice + recomputes
        // dependents (ADR-012), then we resync the render cache. A move that
        // would make two seeds coincident is rejected by the engine, so the
        // handle snaps back (cache left unchanged). The handles draw last so
        // they sit on top of the cell fills and selection stroke.
        // **T-008 (ADR-014):** skipped entirely when the sheet is frozen —
        // no handles drawn, no `ui.interact` (so no drag possible). The
        // widget pass also treats `handle_visible = false` while frozen.
        let frozen = self.engine.voronoi_frozen(self.voronoi.sheet_id);
        if !frozen {
            let seeds: Vec<[f32; 2]> = self
                .voronoi_lattice
                .seeds
                .iter()
                .map(|s| [s.x, s.y])
                .collect();
            let bounds = [
                self.voronoi_lattice.bounds.min.x,
                self.voronoi_lattice.bounds.min.y,
                self.voronoi_lattice.bounds.max.x,
                self.voronoi_lattice.bounds.max.y,
            ];
            let radius = 5.0;
            // First pass: interact to learn which handle (if any) is being
            // dragged this frame. We need this BEFORE drawing so the
            // dragged>fade>no-hover priority can be computed (ADR-014).
            let seeds_screen: Vec<egui::Pos2> = seeds
                .iter()
                .map(|s| egui::pos2(origin.x + s[0], origin.y + s[1]))
                .collect();
            let handle_responses: Vec<egui::Response> = seeds_screen
                .iter()
                .enumerate()
                .map(|(i, &center)| {
                    let handle_rect = egui::Rect::from_center_size(
                        center,
                        egui::vec2(2.0 * radius, 2.0 * radius),
                    );
                    ui.interact(
                        handle_rect,
                        ui.id().with(("vor_seed", i)),
                        egui::Sense::drag(),
                    )
                })
                .collect();
            let dragging_index = handle_responses.iter().position(|r| r.dragged());
            let cursor_pos = response.hover_pos();
            let alphas = seed_handle_alphas(cursor_pos, &seeds_screen, dragging_index);

            let mut pending_drag: Option<(usize, [f32; 2])> = None;
            for (i, center) in seeds_screen.iter().copied().enumerate() {
                let alpha = alphas[i];
                if alpha == 0 {
                    // Skip rendering entirely when invisible — keeps the
                    // hit area alive (the ui.interact above already ran)
                    // but no drawing cost.
                } else {
                    let fill = if Some(i) == dragging_index {
                        egui::Color32::from_rgba_premultiplied(70, 120, 220, alpha)
                    } else {
                        egui::Color32::from_rgba_premultiplied(40, 40, 50, alpha)
                    };
                    painter.circle(
                        center,
                        radius,
                        fill,
                        egui::Stroke::new(
                            1.5_f32,
                            egui::Color32::from_rgba_premultiplied(255, 255, 255, alpha),
                        ),
                    );
                }
                let resp = &handle_responses[i];
                if resp.dragged() {
                    let d = resp.drag_delta();
                    if d != egui::Vec2::ZERO {
                        pending_drag = Some((i, [d.x, d.y]));
                    }
                }
            }
            if let Some((i, delta)) = pending_drag {
                let moved = apply_seed_drag(&seeds, i, delta, bounds);
                if self
                    .engine
                    .set_voronoi_seeds(self.voronoi.sheet_id, moved)
                    .is_ok()
                {
                    self.voronoi_lattice =
                        synced_voronoi_lattice(&self.engine, self.voronoi.sheet_id);
                    self.mark_dirty();
                }
            }
        }

        // In-cell edit overlay — a small text field centred on the
        // selected cell's centroid.
        if let Some(edit) = &mut self.edit {
            let centroid = self.voronoi_lattice.centroid(self.voronoi.selection.cursor);
            let center = egui::pos2(origin.x + centroid.x, origin.y + centroid.y);
            let rect = egui::Rect::from_center_size(center, egui::vec2(160.0, 24.0));
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
