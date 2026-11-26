//! Floating dialog windows (Conditional formatting, Find/Replace, Help, About, Notes).

use carbide_core::SheetId;
use carbide_tess::hex::{self, HexCoord};
use carbide_tess::triangle::TriCoord;
use carbide_tess::voronoi::VoronoiCoord;
use eframe::egui;

use crate::conditional;
use crate::find;
use crate::grid;
use crate::keymap;

use super::helpers::{commit_source, describe_condition, hex_address, triangle_address};
use super::types::{ActiveSheet, CellId, CondDraft, CondKind};
use super::voronoi_helpers::voronoi_address;
use super::{CarbideApp, COLS, HEX_VIEW_RADIUS, ROWS, TRIANGLE_RADIUS};

impl CarbideApp {
    /// The conditional-formatting rule editor — a floating window listing
    /// the square sheet's rules, with a row to add another.
    pub(super) fn conditional_window(&mut self, ctx: &egui::Context) {
        let mut open = self.cond_window_open;
        egui::Window::new("Conditional formatting")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                if self.cond_rules.is_empty() {
                    ui.label(egui::RichText::new("No rules yet — add one below.").weak());
                }
                let count = self.cond_rules.len();
                let mut remove = None;
                let mut edit = None;
                let mut raise = None;
                let mut lower = None;
                for (i, rule) in self.cond_rules.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if let Some(fill) = rule.format.fill {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, fill);
                        }
                        // Render the description in the rule's own style,
                        // so the row previews what a match looks like.
                        let mut desc = egui::RichText::new(describe_condition(&rule.condition));
                        if rule.format.bold {
                            desc = desc.strong();
                        }
                        if rule.format.italic {
                            desc = desc.italics();
                        }
                        if rule.format.strikethrough {
                            desc = desc.strikethrough();
                        }
                        if rule.format.underline {
                            desc = desc.underline();
                        }
                        if let Some(color) = rule.format.text_color {
                            desc = desc.color(color);
                        }
                        ui.label(desc);
                        if ui
                            .add_enabled(i > 0, egui::Button::new("↑").small())
                            .clicked()
                        {
                            raise = Some(i);
                        }
                        if ui
                            .add_enabled(i + 1 < count, egui::Button::new("↓").small())
                            .clicked()
                        {
                            lower = Some(i);
                        }
                        if ui.small_button("Edit").clicked() {
                            edit = Some(i);
                        }
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                // Editing a rule loads it back into the draft and drops it,
                // so the "Add rule" button re-commits the edited form.
                if let Some(i) = edit {
                    self.cond_draft = CondDraft::from_rule(&self.cond_rules[i]);
                    self.cond_rules.remove(i);
                } else if let Some(i) = remove {
                    self.cond_rules.remove(i);
                } else if let Some(i) = raise {
                    if let Some((a, b)) = conditional::swap_for_move(count, i, true) {
                        self.cond_rules.swap(a, b);
                    }
                } else if let Some(i) = lower {
                    if let Some((a, b)) = conditional::swap_for_move(count, i, false) {
                        self.cond_rules.swap(a, b);
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("when value")
                        .selected_text(self.cond_draft.kind.label())
                        .show_ui(ui, |ui| {
                            for kind in CondKind::ALL {
                                ui.selectable_value(&mut self.cond_draft.kind, kind, kind.label());
                            }
                        });
                    if self.cond_draft.kind.needs_threshold() {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cond_draft.threshold)
                                .desired_width(56.0),
                        );
                    }
                    if self.cond_draft.kind.needs_threshold2() {
                        ui.label("and");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cond_draft.threshold2)
                                .desired_width(56.0),
                        );
                    }
                    ui.label("fill");
                    ui.color_edit_button_srgba(&mut self.cond_draft.fill);
                    ui.checkbox(&mut self.cond_draft.bold, "bold");
                    ui.checkbox(&mut self.cond_draft.italic, "italic");
                    ui.checkbox(&mut self.cond_draft.strikethrough, "strike");
                    ui.checkbox(&mut self.cond_draft.underline, "underline");
                    ui.checkbox(&mut self.cond_draft.text_color_on, "text");
                    ui.color_edit_button_srgba(&mut self.cond_draft.text_color);
                    if ui.button("Add rule").clicked() {
                        if let Some(rule) = self.cond_draft.build() {
                            self.cond_rules.push(rule);
                        }
                    }
                });
            });
        self.cond_window_open = open;
    }

    /// The Find panel — a floating window with a query box, prev/next,
    /// and a match count. Ctrl+F opens it.
    pub(super) fn find_window(&mut self, ctx: &egui::Context) {
        if !self.find_open {
            return;
        }
        let mut open = self.find_open;
        egui::Window::new("Find")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.find.query)
                            .desired_width(180.0)
                            .hint_text("find in cells"),
                    );
                    if self.find_just_opened {
                        response.request_focus();
                        self.find_just_opened = false;
                    }
                    if response.changed() {
                        self.refresh_find();
                    }
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        response.request_focus();
                        self.find_step(true);
                    }
                    if ui
                        .checkbox(&mut self.find.case_sensitive, "Match case")
                        .changed()
                    {
                        self.refresh_find();
                    }
                    if ui
                        .checkbox(&mut self.find.whole_cell, "Whole cell")
                        .changed()
                    {
                        self.refresh_find();
                    }
                    if ui
                        .checkbox(&mut self.find.in_selection, "Within selection")
                        .changed()
                    {
                        self.refresh_find();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Replace");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.find.replace)
                            .desired_width(180.0)
                            .hint_text("replace with"),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("◀ Prev").clicked() {
                        self.find_step(false);
                    }
                    if ui.button("Next ▶").clicked() {
                        self.find_step(true);
                    }
                    let count = self.find.match_count();
                    let label = if count > 0 {
                        format!("{} of {}", self.find.current_index(), count)
                    } else if self.find.query.is_empty() {
                        String::new()
                    } else {
                        "no matches".to_string()
                    };
                    ui.label(egui::RichText::new(label).weak());
                });
                ui.horizontal(|ui| {
                    let has_match = self.find.match_count() > 0;
                    if ui
                        .add_enabled(has_match, egui::Button::new("Replace"))
                        .clicked()
                    {
                        self.replace_current();
                    }
                    if ui
                        .add_enabled(has_match, egui::Button::new("Replace All"))
                        .clicked()
                    {
                        self.replace_all_matches();
                    }
                });
            });
        let was_open = self.find_open;
        self.find_open = open;
        // Closing Find drops the query so its grid highlights clear.
        if was_open && !self.find_open {
            self.find.clear();
        }
    }

    /// The sheet, address and current source of a Find match.
    fn cell_id_target(&self, id: CellId) -> (SheetId, String, String) {
        match id {
            CellId::Square((c, r)) => (
                self.square.sheet_id,
                grid::cell_address(c, r),
                self.cell_source(c, r),
            ),
            CellId::Hex(h) => (self.hex.sheet_id, hex_address(h), self.hex_cell_source(h)),
            CellId::Triangle(t) => (
                self.triangle.sheet_id,
                triangle_address(t),
                self.triangle_cell_source(t),
            ),
            CellId::Voronoi(v) => (
                self.voronoi.sheet_id,
                voronoi_address(v),
                self.voronoi_cell_source(v),
            ),
        }
    }

    /// Move the active sheet's selection onto a Find match.
    fn jump_to(&mut self, id: CellId) {
        match id {
            CellId::Square(cell) => self.square.selection.collapse_to(cell),
            CellId::Hex(coord) => self.hex.selection.collapse_to(coord),
            CellId::Triangle(coord) => self.triangle.selection.collapse_to(coord),
            CellId::Voronoi(coord) => self.voronoi.selection.collapse_to(coord),
        }
    }

    /// Rebuild the Find matches from the active sheet's current contents
    /// and jump the selection to the first match.
    pub(super) fn refresh_find(&mut self) {
        let in_selection = self.find.in_selection;
        let cells: Vec<(CellId, String)> = match self.active {
            ActiveSheet::Square => (0..ROWS)
                .flat_map(|r| (0..COLS).map(move |c| (c, r)))
                .filter(|&(c, r)| !in_selection || self.square.selection.contains((c, r)))
                .map(|(c, r)| (CellId::Square((c, r)), self.cell_source(c, r)))
                .collect(),
            ActiveSheet::Hex => hex::hex_disc(HexCoord::new(0, 0), HEX_VIEW_RADIUS)
                .into_iter()
                .filter(|&coord| !in_selection || self.hex.selection.contains(coord))
                .map(|coord| (CellId::Hex(coord), self.hex_cell_source(coord)))
                .collect(),
            ActiveSheet::Triangle => {
                let r = TRIANGLE_RADIUS;
                (-r..=r)
                    .flat_map(|row| (-r..=r).map(move |col| TriCoord::new(col, row)))
                    .filter(|&coord| !in_selection || self.triangle.selection.contains(coord))
                    .map(|coord| (CellId::Triangle(coord), self.triangle_cell_source(coord)))
                    .collect()
            }
            ActiveSheet::Voronoi => (0..self.voronoi_lattice.len() as u32)
                .map(VoronoiCoord)
                .filter(|&coord| !in_selection || self.voronoi.selection.contains(coord))
                .map(|coord| (CellId::Voronoi(coord), self.voronoi_cell_source(coord)))
                .collect(),
        };
        self.find.refresh(cells.into_iter());
        if let Some(id) = self.find.current_match() {
            self.jump_to(id);
        }
    }

    /// Step Find to the next/previous match and move the selection to it.
    pub(super) fn find_step(&mut self, forward: bool) {
        if let Some(id) = self.find.step(forward) {
            self.jump_to(id);
        }
    }

    /// The keyboard-shortcuts help overlay — a floating window listing
    /// `keymap::SHORTCUTS`. Opened by F1 or the ribbon's "?" button.
    pub(super) fn help_window(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let mut open = self.help_open;
        egui::Window::new("Help")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Keyboard");
                egui::Grid::new("shortcuts_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        for &(keys, desc) in keymap::SHORTCUTS {
                            ui.monospace(keys);
                            ui.label(desc);
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                ui.heading("Mouse");
                egui::Grid::new("gestures_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        for &(gesture, desc) in keymap::GESTURES {
                            ui.monospace(gesture);
                            ui.label(desc);
                            ui.end_row();
                        }
                    });
            });
        self.help_open = open;
    }

    /// The About window — a small overlay describing the app, its
    /// stack, and its main capabilities. Opened from Help > About.
    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = self.about_open;
        egui::Window::new("About Carbide")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Carbide");
                ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).weak());
                ui.add_space(6.0);
                ui.label(
                    "A pure-Rust spreadsheet with non-square tessellating cells \
                     (squares, hexagons, triangles, Voronoi) and a DAG-evaluated formula core.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Built with egui / eframe; compiles native and to \
                     WebAssembly. Engine: carbide-core / -tess / -formula / -store.",
                );
                ui.add_space(8.0);
                ui.hyperlink_to("Source", "https://github.com/crussella0129/carbide");
            });
        self.about_open = open;
    }

    /// The cell-note editor — a small window with a multiline field for
    /// the note on `note_cell`, opened from the right-click menu.
    pub(super) fn note_window(&mut self, ctx: &egui::Context) {
        if !self.note_open {
            return;
        }
        let mut open = self.note_open;
        let cell = self.note_cell;
        let address = match cell {
            CellId::Square((c, r)) => grid::cell_address(c, r),
            CellId::Hex(h) => hex_address(h),
            CellId::Triangle(t) => triangle_address(t),
            CellId::Voronoi(v) => voronoi_address(v),
        };
        egui::Window::new("Cell note")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Note for {address}"));
                ui.add(
                    egui::TextEdit::multiline(&mut self.note_draft)
                        .desired_rows(4)
                        .desired_width(260.0),
                );
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.set_note(cell, self.note_draft.clone());
                        self.note_open = false;
                    }
                    if ui.button("Clear").clicked() {
                        self.set_note(cell, String::new());
                        self.note_open = false;
                    }
                });
            });
        // The window's [x] clears `open`; a Save/Clear button may have
        // already closed it.
        self.note_open &= open;
    }

    /// Write a note to whichever sheet `cell` belongs to. Blank text
    /// clears the note (see [`NoteMap::set`]).
    pub(super) fn set_note(&mut self, cell: CellId, text: String) {
        match cell {
            CellId::Square(c) => self.notes.set(c, text),
            CellId::Hex(h) => self.hex_notes.set(h, text),
            CellId::Triangle(t) => self.triangle_notes.set(t, text),
            // Voronoi notes deferred to v151 — set_note on a Voronoi
            // cell is a no-op for now. The note window's address path
            // already handles CellId::Voronoi.
            CellId::Voronoi(_) => {}
        }
    }

    /// Replace the query with the replacement in the current match's
    /// source, write it back, then rebuild the match list.
    fn replace_current(&mut self) {
        if self.find.query.is_empty() {
            return;
        }
        if let Some(id) = self.find.current_match() {
            let (sheet, addr, source) = self.cell_id_target(id);
            let new = find::replace_all(
                &source,
                &self.find.query,
                &self.find.replace,
                self.find.case_sensitive,
            );
            if new != source {
                self.apply_edits(sheet, vec![(addr, commit_source(&new))]);
            }
            self.refresh_find();
        }
    }

    /// Replace the query with the replacement in every matching cell's
    /// source — one undo step — then rebuild the match list.
    fn replace_all_matches(&mut self) {
        if self.find.query.is_empty() {
            return;
        }
        let query = self.find.query.clone();
        let replacement = self.find.replace.clone();
        let case_sensitive = self.find.case_sensitive;
        let matches: Vec<CellId> = self.find.matches().to_vec();
        let mut sheet = self.square.sheet_id;
        let mut targets = Vec::new();
        for id in matches {
            let (s, addr, source) = self.cell_id_target(id);
            sheet = s;
            let new = find::replace_all(&source, &query, &replacement, case_sensitive);
            if new != source {
                targets.push((addr, commit_source(&new)));
            }
        }
        self.apply_edits(sheet, targets);
        self.refresh_find();
    }
}
