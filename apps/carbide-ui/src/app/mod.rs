//! The Carbide application — an `eframe::App` that owns a
//! `WorkbookEngine` and draws the spreadsheet with egui.
//!
//! The square sheet has keyboard navigation, in-cell editing, row/column
//! sizing, cell formatting, a formatting ribbon, multi-cell range
//! selection, copy/paste and (v10) undo/redo. The hex sheet renders
//! `carbide-tess`'s `HexLattice` as a real tessellation and is
//! interactive too — both sheets share the pure `keymap` command layer.

mod cells;
mod commands;
mod demos;
mod dialogs;
mod edit;
mod helpers;
mod io;
mod render_hex;
mod render_square;
mod render_triangle;
mod render_voronoi;
#[cfg(test)]
mod tests;
mod types;
mod voronoi_helpers;

use helpers::{commit_source, engine_label, format_number, picker_label};
use types::{Action, ActiveSheet, CellId, CondDraft, EditState, FillDrag, HeaderDrag, Resize};

use carbide_core::EngineKind;
use carbide_formula::WorkbookEngine;
use carbide_tess::hex::{HexCoord, HexLattice, HexOrientation};
use carbide_tess::triangle::{TriCoord, TriangleLattice};
use carbide_tess::voronoi::{VoronoiCoord, VoronoiLattice};
use carbide_tess::Lattice;
use eframe::egui;

use crate::clipboard::Clipboard;
use crate::conditional::Rule;
use crate::find::FindState;
use crate::format::CellFormat;
use crate::grid::{self, GridMetrics};
use crate::history::History;
use crate::note::NoteMap;
use crate::ribbon;
use crate::selection::Sheet;
use crate::stats;
use crate::widget::Widgets;

const COLS: u32 = 52;
const ROWS: u32 = 200;
/// How many rows a Page Up / Page Down moves the cursor.
const PAGE_ROWS: u32 = 16;

/// Circumradius of a rendered hex cell, in points.
const HEX_SIZE: f32 = 36.0;
const TRIANGLE_SIDE: f32 = 56.0;
/// How many triangle rows/columns to draw above/below/around the origin.
const TRIANGLE_RADIUS: i32 = 6;
/// How many rings of hexes the hex view shows around the origin.
const HEX_VIEW_RADIUS: i32 = 3;

/// How close in time (seconds) two format edits must be to coalesce into
/// one undo step — long enough to catch a colour-picker drag, short
/// enough that two deliberate edits stay separate.
const COALESCE_WINDOW: f64 = 0.6;

/// Ctrl+Shift, for the alignment shortcuts.
const CTRL_SHIFT: egui::Modifiers = egui::Modifiers {
    alt: false,
    ctrl: true,
    shift: true,
    mac_cmd: false,
    command: false,
};

/// Key combos handled while navigating. Each is checked with
/// `consume_key`, so egui's own widgets never also see them.
const NAV_KEYS: &[(egui::Modifiers, egui::Key)] = &[
    (egui::Modifiers::NONE, egui::Key::ArrowUp),
    (egui::Modifiers::NONE, egui::Key::ArrowDown),
    (egui::Modifiers::NONE, egui::Key::ArrowLeft),
    (egui::Modifiers::NONE, egui::Key::ArrowRight),
    (egui::Modifiers::SHIFT, egui::Key::ArrowUp),
    (egui::Modifiers::SHIFT, egui::Key::ArrowDown),
    (egui::Modifiers::SHIFT, egui::Key::ArrowLeft),
    (egui::Modifiers::SHIFT, egui::Key::ArrowRight),
    (egui::Modifiers::CTRL, egui::Key::ArrowUp),
    (egui::Modifiers::CTRL, egui::Key::ArrowDown),
    (egui::Modifiers::CTRL, egui::Key::ArrowLeft),
    (egui::Modifiers::CTRL, egui::Key::ArrowRight),
    (CTRL_SHIFT, egui::Key::ArrowUp),
    (CTRL_SHIFT, egui::Key::ArrowDown),
    (CTRL_SHIFT, egui::Key::ArrowLeft),
    (CTRL_SHIFT, egui::Key::ArrowRight),
    (egui::Modifiers::NONE, egui::Key::Tab),
    (egui::Modifiers::SHIFT, egui::Key::Tab),
    (egui::Modifiers::NONE, egui::Key::Enter),
    (egui::Modifiers::SHIFT, egui::Key::Enter),
    (egui::Modifiers::NONE, egui::Key::Home),
    (egui::Modifiers::CTRL, egui::Key::Home),
    (egui::Modifiers::NONE, egui::Key::End),
    (egui::Modifiers::CTRL, egui::Key::End),
    (egui::Modifiers::NONE, egui::Key::PageUp),
    (egui::Modifiers::NONE, egui::Key::PageDown),
    (egui::Modifiers::NONE, egui::Key::Escape),
    (egui::Modifiers::NONE, egui::Key::F2),
    (egui::Modifiers::NONE, egui::Key::F3),
    (egui::Modifiers::SHIFT, egui::Key::F3),
    (egui::Modifiers::NONE, egui::Key::F1),
    (egui::Modifiers::NONE, egui::Key::Delete),
    (egui::Modifiers::NONE, egui::Key::Backspace),
    (egui::Modifiers::CTRL, egui::Key::A),
    (egui::Modifiers::CTRL, egui::Key::B),
    (egui::Modifiers::CTRL, egui::Key::I),
    (egui::Modifiers::CTRL, egui::Key::U),
    (egui::Modifiers::CTRL, egui::Key::C),
    (egui::Modifiers::CTRL, egui::Key::V),
    (egui::Modifiers::CTRL, egui::Key::X),
    (egui::Modifiers::CTRL, egui::Key::Z),
    (CTRL_SHIFT, egui::Key::Z),
    (egui::Modifiers::CTRL, egui::Key::Y),
    (egui::Modifiers::CTRL, egui::Key::D),
    (egui::Modifiers::CTRL, egui::Key::R),
    (egui::Modifiers::CTRL, egui::Key::F),
    (CTRL_SHIFT, egui::Key::V),
    (CTRL_SHIFT, egui::Key::L),
    (CTRL_SHIFT, egui::Key::E),
    (CTRL_SHIFT, egui::Key::R),
    (CTRL_SHIFT, egui::Key::Num8),
];

/// Key combos handled while editing. Arrows and text are deliberately
/// absent — they belong to the `TextEdit` (caret movement, typing).
const EDIT_KEYS: &[(egui::Modifiers, egui::Key)] = &[
    (egui::Modifiers::NONE, egui::Key::Enter),
    (egui::Modifiers::SHIFT, egui::Key::Enter),
    (egui::Modifiers::NONE, egui::Key::Tab),
    (egui::Modifiers::SHIFT, egui::Key::Tab),
    (egui::Modifiers::NONE, egui::Key::Escape),
];

/// The Carbide front-end application.
pub struct CarbideApp {
    engine: WorkbookEngine,
    /// The square-lattice sheet — engine handle, selection, formula
    /// reference, and per-cell formatting bundled per
    /// [`crate::selection::Sheet`].
    square: Sheet<(u32, u32)>,
    /// The hex-lattice sheet — analogue of [`Self::square`].
    hex: Sheet<HexCoord>,
    /// The triangle-lattice sheet — analogue of [`Self::square`].
    triangle: Sheet<TriCoord>,
    /// The Voronoi-lattice sheet — analogue of [`Self::square`].
    /// Sprint 6 ships single-cell selection only (no range / fill drag).
    voronoi: Sheet<VoronoiCoord>,
    /// Geometry for the hex sheet — owned by `carbide-tess`.
    hex_lattice: HexLattice,
    /// Geometry for the triangle sheet — owned by `carbide-tess`.
    triangle_lattice: TriangleLattice,
    /// Geometry for the Voronoi sheet — carries seeds + bounding rect.
    voronoi_lattice: VoronoiLattice,
    /// `Some(start_pos)` while a marquee drag is in progress on the Voronoi
    /// panel. Cleared at `drag_stopped` (the marquee rect disappears, but the
    /// selected cells stay outlined via `selection.extra` until the next
    /// `collapse_to`). ADR-013.
    voronoi_marquee_start: Option<egui::Pos2>,
    /// Recently-visited Voronoi cells, used by Enter-advance (ADR-014 F3).
    /// Capped at `N-1` where `N` = current seed count. Cleared on primary
    /// cell-select click (T-010 C-009) and on workbook reload / seed-count
    /// change (T-010 C-005 — see `rebind_sheet_ids`).
    voronoi_visit_history: std::collections::VecDeque<VoronoiCoord>,
    /// Which sheet is on screen.
    active: ActiveSheet,
    /// The square cursor as of the last frame — drives scroll-to-cursor.
    prev_cursor: (u32, u32),
    /// `Some` while a cell is being edited (on whichever sheet is active).
    edit: Option<EditState>,
    /// Per-column widths and per-row heights of the square sheet.
    metrics: GridMetrics,
    /// `Some` while a header border is being dragged.
    resizing: Option<Resize>,
    /// The pointer position when the user pressed the button — `Some`
    /// while a press is active. Captured BEFORE egui's drag threshold
    /// is crossed, so the resize hit-test reflects where the user
    /// clicked rather than the position the pointer has moved to by
    /// the time `drag_started` fires (which can be 5+ pixels off the
    /// border).
    press_pos: Option<egui::Pos2>,
    /// `Some` while a column or row header is being dragged to sweep a
    /// range selection.
    header_drag: Option<HeaderDrag>,
    /// `Some` while the format-painter is armed: the captured format
    /// that the next square-cell click will apply.
    format_painter: Option<CellFormat>,
    /// `Some` while a fill-handle drag is in progress on the square
    /// sheet — see [`FillDrag`].
    fill_drag: Option<FillDrag>,
    /// The copy / cut / paste object — the captured block, and its cut
    /// origin when it was cut rather than copied.
    clipboard: Clipboard,
    /// Undo/redo history — one entry per action.
    history: History<Action>,
    /// `ctx.input().time` for the current frame — the clock used to
    /// coalesce rapid format edits.
    frame_time: f64,
    /// When the last format action was recorded, and the cells it
    /// touched (on any lattice). A fresh format edit over the same cells
    /// within `COALESCE_WINDOW` merges into it — so a colour-picker drag
    /// is one undo step, not one per frame.
    last_format_time: f64,
    last_format_cells: Vec<CellId>,
    /// Conditional-formatting rules for the square sheet, evaluated each
    /// frame and layered over a cell's manual format.
    cond_rules: Vec<Rule>,
    /// Whether the conditional-formatting rule editor window is open.
    cond_window_open: bool,
    /// The in-progress new rule in that editor.
    cond_draft: CondDraft,
    /// Square-sheet cells that render as interactive boolean toggles.
    square_widgets: Widgets<(u32, u32)>,
    /// Hex-sheet widgets. Mirrors `square_widgets` but keyed by
    /// [`HexCoord`]. Populated by the Hex Game demo seed and by future
    /// per-lattice ribbon paths.
    hex_widgets: Widgets<HexCoord>,
    /// Triangle-sheet widgets. Mirrors `square_widgets`/`hex_widgets`
    /// but keyed by [`TriCoord`]. Closes the ADR-005 per-lattice
    /// symmetry — the triangle sheet now carries the same widget
    /// surface as the other two. Same Button + Toggle support; Slider
    /// and ProgressBar fall through to text render (ADR-006).
    triangle_widgets: Widgets<TriCoord>,
    /// Voronoi-sheet widgets. The fourth and final lattice to gain the
    /// `Widgets<K>` surface (closes the ADR-005 arc). Button + Toggle
    /// render inscribed in each cell's polygon; Slider/ProgressBar fall
    /// through to text (ADR-006).
    voronoi_widgets: Widgets<VoronoiCoord>,
    /// The formula bar's edit buffer — mirrors the active cell's source
    /// except while the bar itself is being edited.
    formula_bar: String,
    /// Find-panel state — the query and its matching cells.
    find: FindState<CellId>,
    /// Whether the Find window is open.
    find_open: bool,
    /// Set when Find is opened by Ctrl+F so the query field grabs focus.
    find_just_opened: bool,
    /// Whether the keyboard-shortcuts help overlay is open.
    help_open: bool,
    /// Whether the About / app-info window is open.
    about_open: bool,
    /// Free-text notes on square-sheet cells.
    notes: NoteMap<(u32, u32)>,
    /// Free-text notes on hex-sheet cells.
    hex_notes: NoteMap<HexCoord>,
    /// Free-text notes on triangle-sheet cells.
    triangle_notes: NoteMap<TriCoord>,
    /// Free-text notes on Voronoi-sheet cells.
    voronoi_notes: NoteMap<VoronoiCoord>,
    /// Whether the cell-note editor window is open.
    note_open: bool,
    /// The cell the note editor is editing — on either sheet.
    note_cell: CellId,
    /// The note editor's text buffer.
    note_draft: String,
    /// The Name box's edit buffer — the active cell's address, editable
    /// to jump the selection (square sheet).
    name_box: String,
    /// Whether the dark colour theme is active (the default).
    dark_mode: bool,
    /// Stage Mode hides the editing chrome (ribbon, formula bar, row
    /// and column headers, sheet tabs) and locks cells against direct
    /// editing — widgets stay interactive. Toggled with Ctrl+Shift+P.
    stage_mode: bool,
    /// Bytes from an Open dialog that resolved asynchronously, waiting to
    /// be applied to the engine on the next frame. egui's `update` can't
    /// `await`, so the file-picker future writes here and the next frame's
    /// `update` drains the slot.
    pending_open_bytes: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
    /// `true` when an autosave would persist new content. Flipped by
    /// `mark_dirty()` at every user-driven mutation; cleared on autosave
    /// fire and on Open. Seed demos do not flip it (they run before `new`
    /// returns).
    dirty: bool,
    /// `ctx.input(|i| i.time)` of the most recent autosave fire — used by
    /// `maybe_autosave` to enforce the 2 s debounce.
    last_autosave: f64,
    /// Window during which autosave is held off — bumped after an Open to
    /// avoid clobbering the just-loaded state with stale dirty.
    suppress_autosave_until: f64,
}

impl eframe::App for CarbideApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Drain any bytes left by an async Open dialog BEFORE handling
        // this frame's input — the restored state then drives the rest
        // of the update.
        self.drain_pending_open();
        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        self.frame_time = ctx.input(|i| i.time);
        for command in self.collect_commands(&ctx) {
            // Every dispatched command is a potential mutation; the
            // central mark_dirty here means we don't have to thread
            // mark_dirty through each command's body. Save/Open/Find/
            // help/etc. flip the bit too but the maybe_autosave debounce
            // makes that a wash — at worst a redundant write 2s later.
            // Save and Open clear the bit themselves on success.
            self.mark_dirty();
            self.apply(command, &ctx);
        }

        // Menu bar above the ribbon — File / Edit / Format / Data /
        // View / Help. Items dispatch through the same `RibbonAction`
        // pipeline so the ribbon and menu share handlers.
        //
        // Stage Mode hides every editing-chrome panel — menu bar,
        // ribbon, formula bar, sheet tabs — so a sheet reads as an
        // app rather than a spreadsheet.
        if !self.stage_mode {
            egui::Panel::top("carbide_menu_bar").show(ui, |ui| {
                let can_undo = self.history.can_undo();
                let can_redo = self.history.can_redo();
                if let Some(action) = ribbon::menu_bar(ui, can_undo, can_redo) {
                    self.apply_ribbon(action, &ctx);
                }
            });
        }

        if !self.stage_mode {
            egui::Panel::top("carbide_ribbon").show(ui, |ui| match self.active {
                ActiveSheet::Square => {
                    let current = self.square.formats.get(self.square.selection.cursor);
                    let can_undo = self.history.can_undo();
                    let can_redo = self.history.can_redo();
                    if let Some(action) = ribbon::ribbon(
                        ui,
                        &current,
                        can_undo,
                        can_redo,
                        self.format_painter.is_some(),
                        false, // voronoi_active
                        false, // voronoi_frozen
                    ) {
                        self.apply_ribbon(action, &ctx);
                    }
                }
                ActiveSheet::Hex => {
                    // The hex-only orientation toggle, then the shared format
                    // ribbon — formatting now applies to hex cells too.
                    ui.horizontal(|ui| {
                        // Switching orientation changes geometry only — axial
                        // (q,r) coords are orientation-independent, so cell
                        // data, formatting and selection carry over.
                        let pointy = matches!(self.hex_lattice.orientation, HexOrientation::Pointy);
                        if ui.selectable_label(pointy, "Pointy-top").clicked() {
                            self.hex_lattice = HexLattice::pointy(HEX_SIZE);
                        }
                        if ui.selectable_label(!pointy, "Flat-top").clicked() {
                            self.hex_lattice = HexLattice::flat(HEX_SIZE);
                        }
                    });
                    let current = self.hex.formats.get(self.hex.selection.cursor);
                    let can_undo = self.history.can_undo();
                    let can_redo = self.history.can_redo();
                    if let Some(action) = ribbon::ribbon(
                        ui,
                        &current,
                        can_undo,
                        can_redo,
                        self.format_painter.is_some(),
                        false, // voronoi_active
                        false, // voronoi_frozen
                    ) {
                        self.apply_ribbon(action, &ctx);
                    }
                }
                ActiveSheet::Triangle => {
                    let current = self.triangle.formats.get(self.triangle.selection.cursor);
                    let can_undo = self.history.can_undo();
                    let can_redo = self.history.can_redo();
                    if let Some(action) = ribbon::ribbon(
                        ui,
                        &current,
                        can_undo,
                        can_redo,
                        self.format_painter.is_some(),
                        false, // voronoi_active
                        false, // voronoi_frozen
                    ) {
                        self.apply_ribbon(action, &ctx);
                    }
                }
                ActiveSheet::Voronoi => {
                    // Contextual Voronoi ribbon group surfaces only here.
                    let current = self.voronoi.formats.get(self.voronoi.selection.cursor);
                    let can_undo = self.history.can_undo();
                    let can_redo = self.history.can_redo();
                    let frozen = self.engine.voronoi_frozen(self.voronoi.sheet_id);
                    if let Some(action) = ribbon::ribbon(
                        ui,
                        &current,
                        can_undo,
                        can_redo,
                        self.format_painter.is_some(),
                        true,
                        frozen,
                    ) {
                        self.apply_ribbon(action, &ctx);
                    }
                }
            });
        }

        if !self.stage_mode {
            egui::Panel::top("carbide_formula_bar")
                .resizable(true)
                .min_size(28.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (addr, source) = self.active_address_and_source();
                        match self.active {
                            ActiveSheet::Square => {
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.name_box)
                                        .desired_width(64.0)
                                        .font(egui::TextStyle::Monospace),
                                );
                                if response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    // Enter jumps the selection to the typed address.
                                    if let Some((c, r)) = grid::parse_address(&self.name_box) {
                                        if c < COLS && r < ROWS {
                                            self.square.selection.collapse_to((c, r));
                                        }
                                    }
                                } else if !response.has_focus() {
                                    self.name_box = addr.clone();
                                }
                            }
                            ActiveSheet::Hex => {
                                ui.monospace(addr.clone());
                            }
                            ActiveSheet::Triangle => {
                                ui.monospace(addr.clone());
                            }
                            ActiveSheet::Voronoi => {
                                // T-007: Voronoi name box — display the active
                                // V(n) address; pressing Enter on a typed
                                // address jumps the cursor there.
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.name_box)
                                        .desired_width(64.0)
                                        .font(egui::TextStyle::Monospace),
                                );
                                if response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if let Ok(coord) =
                                        self.voronoi_lattice.parse_address(&self.name_box)
                                    {
                                        self.voronoi.selection.collapse_to(coord);
                                    }
                                } else if !response.has_focus() {
                                    self.name_box = addr.clone();
                                }
                            }
                        }
                        match self.active {
                            ActiveSheet::Square if self.square.selection.is_range() => {
                                let (cols, rows) = self.square.selection.dimensions();
                                ui.label(egui::RichText::new(format!("{cols}C × {rows}R")).weak());
                            }
                            ActiveSheet::Hex if self.hex.selection.is_range() => {
                                let (q, r) = self.hex.selection.dimensions();
                                ui.label(egui::RichText::new(format!("{q}q × {r}r")).weak());
                            }
                            ActiveSheet::Voronoi => {
                                // Marquee dimension hint: deduplicated count across
                                // anchor + extra, shown only when > 1 cell is selected.
                                let n = self.voronoi.selection.cells().len();
                                if n > 1 {
                                    ui.label(egui::RichText::new(format!("{n} cells")).weak());
                                }
                            }
                            _ => {}
                        }
                        // Per-cell language picker. Shows the active cell's
                        // engine: an explicit override, or `None` meaning
                        // "inherit the workbook default". Picking "Default"
                        // clears any override; picking a specific engine
                        // sets one.
                        let (sheet, picker_addr) = self.active_target();
                        let snapshot = self.engine.get_cell(sheet, &picker_addr);
                        let cell_override: Option<EngineKind> =
                            snapshot.as_ref().and_then(|s| s.engine);
                        let default_engine = self.engine.default_engine();
                        let mut new_choice: Option<EngineKind> = cell_override;
                        let display = picker_label(cell_override, default_engine);
                        egui::ComboBox::from_id_salt("language_picker")
                            .selected_text(display)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut new_choice,
                                    None,
                                    format!("Default ({})", engine_label(default_engine, true)),
                                );
                                ui.separator();
                                for kind in [
                                    EngineKind::ExcelLite,
                                    EngineKind::Python,
                                    EngineKind::Rhai,
                                    EngineKind::RustNative,
                                ] {
                                    ui.selectable_value(
                                        &mut new_choice,
                                        Some(kind),
                                        engine_label(kind, false),
                                    );
                                }
                            });
                        if new_choice != cell_override {
                            let _ = self.engine.set_cell_engine(sheet, &picker_addr, new_choice);
                        }
                        ui.separator();
                        let width = ui.available_width();
                        let height = ui.available_height();
                        // Multiline so long formulas wrap and the panel-drag
                        // resize actually adds usable space. Tab or click-outside
                        // commits (the singleline path used Enter, but multiline
                        // reserves Enter for newline so we don't override it).
                        let response = ui.add_sized(
                            [width, height.max(20.0)],
                            egui::TextEdit::multiline(&mut self.formula_bar)
                                .desired_width(width)
                                .desired_rows(1)
                                .font(egui::TextStyle::Monospace)
                                .hint_text(
                                    "value or =formula — drag the bottom of the bar to expand",
                                ),
                        );
                        if response.lost_focus() {
                            // Click-outside or Tab commits the bar to the cell.
                            self.edit = None;
                            let (sheet, addr) = self.active_target();
                            let new_source = commit_source(&self.formula_bar);
                            self.apply_edits(sheet, vec![(addr, new_source)]);
                        } else if !response.has_focus() {
                            // Not being edited — mirror the active cell's source.
                            self.formula_bar = source;
                        }
                    });
                });
        }

        if !self.stage_mode {
            egui::Panel::bottom("carbide_sheet_tabs").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.active == ActiveSheet::Square, "Budget")
                        .clicked()
                    {
                        self.commit_edit();
                        self.active = ActiveSheet::Square;
                        if self.find_open {
                            self.refresh_find();
                        }
                    }
                    if ui
                        .selectable_label(self.active == ActiveSheet::Hex, "Hex Game")
                        .clicked()
                    {
                        self.commit_edit();
                        self.active = ActiveSheet::Hex;
                        if self.find_open {
                            self.refresh_find();
                        }
                    }
                    if ui
                        .selectable_label(self.active == ActiveSheet::Triangle, "Tri demo")
                        .clicked()
                    {
                        self.commit_edit();
                        self.active = ActiveSheet::Triangle;
                        if self.find_open {
                            self.refresh_find();
                        }
                    }
                    if ui
                        .selectable_label(self.active == ActiveSheet::Voronoi, "Voronoi")
                        .clicked()
                    {
                        self.commit_edit();
                        self.active = ActiveSheet::Voronoi;
                        if self.find_open {
                            self.refresh_find();
                        }
                    }
                    // Selection statistics, pushed to the right edge.
                    let stats = stats::selection_stats(&self.selection_values());
                    if stats.nonempty > 0 {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let text = if stats.count > 0 {
                                let avg = stats.average.map(format_number).unwrap_or_default();
                                let min = stats.min.map(format_number).unwrap_or_default();
                                let max = stats.max.map(format_number).unwrap_or_default();
                                format!(
                                    "Sum {}     Avg {}     Min {}     Max {}     Count {}",
                                    format_number(stats.sum),
                                    avg,
                                    min,
                                    max,
                                    stats.nonempty,
                                )
                            } else {
                                format!("Count {}", stats.nonempty)
                            };
                            ui.label(egui::RichText::new(text).weak());
                        });
                    }
                });
            });
        }

        // In Stage Mode, paint a small floating "Exit" button so the
        // user always has a visible way out (Esc also exits — see
        // the `ClearMarquee` handler).
        if self.stage_mode {
            let exit_area = egui::Area::new(egui::Id::unique("carbide_stage_exit"))
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
                .interactable(true);
            exit_area.show(&ctx, |ui| {
                if ui
                    .add(egui::Button::new("Exit Stage Mode").min_size(egui::vec2(130.0, 28.0)))
                    .clicked()
                {
                    self.stage_mode = false;
                }
            });
        }

        egui::CentralPanel::default().show(ui, |ui| match self.active {
            ActiveSheet::Square => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.draw_grid(ui));
            }
            ActiveSheet::Hex => self.draw_hex_grid(ui),
            ActiveSheet::Triangle => self.draw_triangle_grid(ui),
            ActiveSheet::Voronoi => self.draw_voronoi_grid(ui),
        });

        self.conditional_window(&ctx);
        self.find_window(&ctx);
        self.help_window(&ctx);
        self.about_window(&ctx);
        self.note_window(&ctx);

        // End of frame — try a debounced autosave. The debounce in
        // `maybe_autosave` means a flurry of edits still results in at
        // most one write every 2 s.
        self.maybe_autosave(self.frame_time);
    }
}
