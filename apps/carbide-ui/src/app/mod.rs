//! The Carbide application — an `eframe::App` that owns a
//! `WorkbookEngine` and draws the spreadsheet with egui.
//!
//! The square sheet has keyboard navigation, in-cell editing, row/column
//! sizing, cell formatting, a formatting ribbon, multi-cell range
//! selection, copy/paste and (v10) undo/redo. The hex sheet renders
//! `carbide-tess`'s `HexLattice` as a real tessellation and is
//! interactive too — both sheets share the pure `keymap` command layer.

mod demos;
mod dialogs;
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

use helpers::*;
use types::*;
use voronoi_helpers::*;

use carbide_core::{CellValue, EngineKind, SheetId};
use carbide_formula::WorkbookEngine;
use carbide_tess::hex::{HexCoord, HexLattice, HexOrientation};
use carbide_tess::triangle::{TriCoord, TriangleLattice};
use carbide_tess::voronoi::{VoronoiCoord, VoronoiLattice};
use carbide_tess::Lattice;
use eframe::egui;

use crate::clipboard::{Clipboard, CopiedCell, PasteMode, SourceLattice};
use crate::conditional::{self, Rule};
use crate::find::FindState;
use crate::format::{self, BorderMode, CellFormat, HexBorders};
use crate::grid::{self, GridMetrics};
use crate::history::History;
use crate::keymap::{self, Command, Dir, Mode};
use crate::note::NoteMap;
use crate::ribbon::{self, RibbonAction};
use crate::selection::{
    FillDir, HexSelection, Selection, Sheet, SquareSelection, TriangleSelection,
};
use crate::sort;
use crate::stats;
use crate::widget::{self, Widgets};

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
    /// touched. A fresh format edit over the same cells within
    /// `COALESCE_WINDOW` merges into it — so a colour-picker drag is one
    /// undo step, not one per frame.
    last_format_time: f64,
    last_format_cells: Vec<(u32, u32)>,
    /// The hex-sheet counterparts, for hex format coalescing.
    last_hex_format_time: f64,
    last_hex_format_cells: Vec<HexCoord>,
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

impl CarbideApp {
    fn mode(&self) -> Mode {
        if self.edit.is_some() {
            Mode::Editing
        } else {
            Mode::Navigating
        }
    }

    /// The display text for a square-sheet cell, read from the engine and
    /// rendered under the cell's number format.
    fn cell_text(&self, col: u32, row: u32) -> String {
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
    fn cell_value(&self, col: u32, row: u32) -> CellValue {
        self.engine
            .get_cell(self.square.sheet_id, &grid::cell_address(col, row))
            .map(|snapshot| snapshot.value)
            .unwrap_or_default()
    }

    /// The raw evaluated value of a hex-sheet cell, for conditional
    /// formatting. `Empty` when the cell holds nothing.
    fn hex_cell_value(&self, coord: HexCoord) -> CellValue {
        self.engine
            .get_cell(self.hex.sheet_id, &hex_address(coord))
            .map(|snapshot| snapshot.value)
            .unwrap_or_default()
    }

    /// The effective format for a hex cell — its stored format layered
    /// with the first matching conditional-formatting rule.
    fn hex_effective_format(&self, coord: HexCoord) -> CellFormat {
        let base = self.hex.formats.get(coord);
        if self.cond_rules.is_empty() {
            base
        } else {
            conditional::effective_format(&base, &self.hex_cell_value(coord), &self.cond_rules)
        }
    }

    /// The raw source text of a square-sheet cell — empty when the cell
    /// holds nothing. Find and Replace both work on this.
    fn cell_source(&self, col: u32, row: u32) -> String {
        self.engine
            .get_cell(self.square.sheet_id, &grid::cell_address(col, row))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The raw source text of a hex-sheet cell — empty when it holds
    /// nothing. Find and Replace use it on the hex sheet.
    fn hex_cell_source(&self, coord: HexCoord) -> String {
        self.engine
            .get_cell(self.hex.sheet_id, &hex_address(coord))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The display text for a hex-sheet cell. (The number format is not
    /// yet applied on the hex sheet — that is a follow-on.)
    fn hex_cell_text(&self, coord: HexCoord) -> String {
        self.engine
            .get_cell(self.hex.sheet_id, &hex_address(coord))
            .map(|snapshot| natural_text(snapshot.value))
            .unwrap_or_default()
    }

    /// The raw source of a triangle-sheet cell.
    fn triangle_cell_source(&self, coord: TriCoord) -> String {
        self.engine
            .get_cell(self.triangle.sheet_id, &triangle_address(coord))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The display text for a triangle-sheet cell.
    fn triangle_cell_text(&self, coord: TriCoord) -> String {
        self.engine
            .get_cell(self.triangle.sheet_id, &triangle_address(coord))
            .map(|snapshot| natural_text(snapshot.value))
            .unwrap_or_default()
    }

    /// The raw source of a Voronoi-sheet cell.
    fn voronoi_cell_source(&self, coord: VoronoiCoord) -> String {
        self.engine
            .get_cell(self.voronoi.sheet_id, &voronoi_address(coord))
            .and_then(|snapshot| snapshot.source)
            .unwrap_or_default()
    }

    /// The display text for a Voronoi-sheet cell.
    fn voronoi_cell_text(&self, coord: VoronoiCoord) -> String {
        self.engine
            .get_cell(self.voronoi.sheet_id, &voronoi_address(coord))
            .map(|snapshot| natural_text(snapshot.value))
            .unwrap_or_default()
    }

    /// The triangle-sheet cell value used by conditional formatting.
    fn triangle_cell_value(&self, coord: TriCoord) -> CellValue {
        self.engine
            .get_cell(self.triangle.sheet_id, &triangle_address(coord))
            .map(|snapshot| snapshot.value)
            .unwrap_or_default()
    }

    /// The effective cell format for a triangle-sheet cell — base
    /// format plus any conditional-rule overrides for the cell's
    /// current value. Cond rules apply uniformly across every sheet.
    fn triangle_effective_format(&self, coord: TriCoord) -> CellFormat {
        let base = self.triangle.formats.get(coord);
        if self.cond_rules.is_empty() {
            base
        } else {
            conditional::effective_format(&base, &self.triangle_cell_value(coord), &self.cond_rules)
        }
    }

    /// The raw source of the active square cell (the selection cursor).
    fn selected_source(&self) -> String {
        let (col, row) = self.square.selection.cursor;
        self.engine
            .get_cell(self.square.sheet_id, &grid::cell_address(col, row))
            .and_then(|s| s.source)
            .unwrap_or_default()
    }

    /// The raw source of the active cell on whichever sheet is active.
    fn active_cell_source(&self) -> String {
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
    fn active_target(&self) -> (SheetId, String) {
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
    fn active_address_and_source(&self) -> (String, String) {
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
    fn selection_values(&self) -> Vec<CellValue> {
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

    /// Read key events and turn them into commands through `keymap`. Keys
    /// that map to a command are consumed so no other widget reacts too.
    fn collect_commands(&self, ctx: &egui::Context) -> Vec<Command> {
        // A focused text input that is not the in-cell editor — the
        // formula bar, or a dialog field — owns the keyboard; don't also
        // read its keystrokes as cell commands.
        if self.edit.is_none() && ctx.wants_keyboard_input() {
            return Vec::new();
        }
        let mode = self.mode();
        let keys = match mode {
            Mode::Editing => EDIT_KEYS,
            Mode::Navigating => NAV_KEYS,
        };
        let mut commands = Vec::new();
        ctx.input_mut(|input| {
            for &(modifiers, key) in keys {
                if input.consume_key(modifiers, key) {
                    if let Some(cmd) =
                        keymap::command_for_key(key, modifiers.shift, modifiers.ctrl, mode)
                    {
                        commands.push(cmd);
                    }
                }
            }
            if mode == Mode::Navigating {
                // The first typed character begins an edit; remove that
                // text event so the new `TextEdit` doesn't re-type it.
                let mut typed: Option<String> = None;
                input.events.retain(|event| match event {
                    egui::Event::Text(text) if typed.is_none() => {
                        typed = Some(text.clone());
                        false
                    }
                    _ => true,
                });
                if let Some(text) = typed {
                    if let Some(cmd) = keymap::command_for_text(&text, mode) {
                        commands.push(cmd);
                    }
                }
            }
        });
        commands
    }

    fn apply(&mut self, command: Command, ctx: &egui::Context) {
        match command {
            Command::Move(dir) => self.move_active(dir),
            Command::Extend(dir) => self.extend_active(dir),
            Command::SelectAll => self.select_all(),
            Command::SelectRegion => self.select_region(),
            Command::Jump(dir) => self.jump_active(dir),
            Command::JumpExtend(dir) => self.jump_extend_active(dir),
            Command::MoveToRowStart => match self.active {
                ActiveSheet::Square => {
                    let row = self.square.selection.cursor.1;
                    self.square.selection.collapse_to((0, row));
                }
                ActiveSheet::Hex => self.hex.selection.collapse_to(HexCoord::new(0, 0)),
                ActiveSheet::Triangle => {
                    let row = self.triangle.selection.cursor.row;
                    self.triangle.selection.collapse_to(TriCoord::new(0, row));
                }
                ActiveSheet::Voronoi => {}
            },
            Command::MoveToOrigin => match self.active {
                ActiveSheet::Square => self.square.selection.collapse_to((0, 0)),
                ActiveSheet::Hex => self.hex.selection.collapse_to(HexCoord::new(0, 0)),
                ActiveSheet::Triangle => self.triangle.selection.collapse_to(TriCoord::new(0, 0)),
                ActiveSheet::Voronoi => {}
            },
            Command::MoveToRowEnd => match self.active {
                ActiveSheet::Square => {
                    let row = self.square.selection.cursor.1;
                    self.square.selection.collapse_to((COLS - 1, row));
                }
                ActiveSheet::Hex => self.hex.selection.collapse_to(HexCoord::new(0, 0)),
                ActiveSheet::Triangle => {
                    let row = self.triangle.selection.cursor.row;
                    self.triangle
                        .selection
                        .collapse_to(TriCoord::new(TRIANGLE_RADIUS, row));
                }
                ActiveSheet::Voronoi => {}
            },
            Command::MoveToSheetEnd => match self.active {
                ActiveSheet::Square => self.square.selection.collapse_to((COLS - 1, ROWS - 1)),
                ActiveSheet::Hex => self.hex.selection.collapse_to(HexCoord::new(0, 0)),
                ActiveSheet::Triangle => self
                    .triangle
                    .selection
                    .collapse_to(TriCoord::new(TRIANGLE_RADIUS, TRIANGLE_RADIUS)),
                ActiveSheet::Voronoi => {}
            },
            Command::PageUp => self.page(true),
            Command::PageDown => self.page(false),
            Command::BeginEdit { replace_with } => self.begin_edit(replace_with),
            Command::Commit(dir) => {
                self.commit_edit();
                self.move_active(dir);
            }
            Command::Cancel => {
                self.edit = None;
                self.square.formula_highlight = None;
                self.square.formula_drag = None;
                self.hex.formula_highlight = None;
                self.hex.formula_drag = None;
                self.triangle.formula_highlight = None;
                self.triangle.formula_drag = None;
            }
            Command::ClearMarquee => {
                // Escape exits Stage Mode first — that's the most
                // load-bearing thing Escape can do when the editing
                // chrome is hidden.
                if self.stage_mode {
                    self.stage_mode = false;
                    return;
                }
                if self.clipboard.cut_origin().is_some() {
                    self.clipboard.consume_cut();
                }
                self.format_painter = None;
            }
            Command::Clear => self.clear_active(),
            Command::ToggleBold => self.toggle_range(|f| f.bold, |f, v| f.bold = v),
            Command::ToggleItalic => self.toggle_range(|f| f.italic, |f, v| f.italic = v),
            Command::ToggleUnderline => self.toggle_range(|f| f.underline, |f, v| f.underline = v),
            Command::SetAlign(align) => self.format_range(|f| f.align = align),
            Command::Copy => self.copy_selection(ctx),
            Command::Cut => self.cut_selection(ctx),
            Command::Paste => self.paste(PasteMode::Normal),
            Command::PasteValues => self.paste(PasteMode::ValuesOnly),
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::FillDown => self.fill(FillDir::Down),
            Command::FillRight => self.fill(FillDir::Right),
            Command::OpenFind => {
                self.find_open = true;
                self.find_just_opened = true;
            }
            Command::FindNext => self.find_step(true),
            Command::FindPrev => self.find_step(false),
            Command::OpenHelp => self.help_open = true,
            Command::ToggleStageMode => {
                // Toggling Stage Mode commits any in-progress edit so
                // the locked-cells invariant holds the moment we enter
                // stage.
                self.commit_edit();
                self.stage_mode = !self.stage_mode;
            }
            Command::ExitStageMode => {
                self.commit_edit();
                self.stage_mode = false;
            }
            // File operations — handlers land in T-101e / T-101f.
            Command::Save => self.handle_save(false),
            Command::SaveAs => self.handle_save(true),
            Command::Open => self.handle_open(),
        }
    }

    /// Select every cell of the active sheet — Ctrl+A, or a click on the
    /// header corner. The hex sheet has no fixed bounds, so it selects
    /// the parallelogram bounding the visible disc.
    fn select_all(&mut self) {
        match self.active {
            ActiveSheet::Square => self.square.selection = Selection::all(COLS, ROWS),
            ActiveSheet::Hex => {
                let r = HEX_VIEW_RADIUS;
                self.hex.selection =
                    HexSelection::from_anchor_cursor(HexCoord::new(-r, -r), HexCoord::new(r, r));
            }
            ActiveSheet::Triangle => {
                let r = TRIANGLE_RADIUS;
                self.triangle.selection = TriangleSelection::from_anchor_cursor(
                    TriCoord::new(-r, -r),
                    TriCoord::new(r, r),
                );
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// Expand the selection to the current data region around the cursor
    /// — the contiguous block of non-empty cells, Excel's "current
    /// region". The active cell lands at the region's top-left.
    fn select_region(&mut self) {
        match self.active {
            ActiveSheet::Square => {
                let ((min_c, min_r), (max_c, max_r)) =
                    grid::current_region(self.square.selection.cursor, COLS, ROWS, |c, r| {
                        self.square_occupied(c, r)
                    });
                self.square.selection =
                    Selection::from_anchor_cursor((max_c, max_r), (min_c, min_r));
            }
            ActiveSheet::Hex => {
                let cursor = self.hex.selection.cursor;
                let ((min_q, min_r), (max_q, max_r)) =
                    hex_current_region((cursor.q, cursor.r), HEX_VIEW_RADIUS, |q, r| {
                        let c = HexCoord::new(q, r);
                        hex_in_view(c) && self.hex_occupied(c)
                    });
                self.hex.selection = HexSelection::from_anchor_cursor(
                    HexCoord::new(max_q, max_r),
                    HexCoord::new(min_q, min_r),
                );
            }
            ActiveSheet::Triangle => {
                // Triangle data-region detection lands in a later
                // pass; for now expanding from the cursor just keeps
                // the cursor put.
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// Apply a formatting action from the ribbon across the selection.
    fn apply_ribbon(&mut self, action: RibbonAction, ctx: &egui::Context) {
        // Same approach as in `update()` — every dispatched ribbon action
        // is a potential mutation; the central mark_dirty keeps the call
        // sites small. Save/Open re-clear the bit on success.
        self.mark_dirty();
        match action {
            RibbonAction::ToggleBold => self.toggle_range(|f| f.bold, |f, v| f.bold = v),
            RibbonAction::ToggleItalic => self.toggle_range(|f| f.italic, |f, v| f.italic = v),
            RibbonAction::ToggleStrikethrough => {
                self.toggle_range(|f| f.strikethrough, |f, v| f.strikethrough = v)
            }
            RibbonAction::ToggleUnderline => {
                self.toggle_range(|f| f.underline, |f, v| f.underline = v)
            }
            RibbonAction::SetAlign(align) => self.format_range(|f| f.align = align),
            RibbonAction::SetVAlign(valign) => self.format_range(|f| f.valign = valign),
            RibbonAction::SetFontSize(size) => self.format_range(|f| f.font_size = size),
            RibbonAction::SetNumber(number) => self.format_range(|f| f.number = number),
            RibbonAction::AdjustDecimals(delta) => {
                self.format_range(|f| f.number = format::adjust_decimals(f.number, delta))
            }
            RibbonAction::SetTextColor(color) => self.format_range(|f| f.text_color = color),
            RibbonAction::SetFill(fill) => self.format_range(|f| f.fill = fill),
            RibbonAction::ClearFormat => self.format_range(|f| *f = CellFormat::default()),
            RibbonAction::Undo => self.undo(),
            RibbonAction::Redo => self.redo(),
            RibbonAction::Copy => self.copy_selection(ctx),
            RibbonAction::Cut => self.cut_selection(ctx),
            RibbonAction::Paste => self.paste(PasteMode::Normal),
            RibbonAction::PasteValues => self.paste(PasteMode::ValuesOnly),
            RibbonAction::OpenConditional => self.cond_window_open = true,
            RibbonAction::ToggleWidget => self.toggle_widget_cells(),
            RibbonAction::ToggleSlider => self.toggle_slider_cells(),
            RibbonAction::ToggleButton => self.toggle_button_cells(),
            RibbonAction::ToggleProgressBar => self.toggle_progress_bar_cells(),
            RibbonAction::Aggregate(func) => self.autosum(func),
            RibbonAction::SetBorders(mode) => self.apply_border(mode),
            RibbonAction::ToggleNegativeRed => {
                self.toggle_range(|f| f.negative_red, |f, v| f.negative_red = v);
            }
            RibbonAction::ToggleWrapText => {
                self.toggle_range(|f| f.wrap_text, |f, v| f.wrap_text = v);
            }
            RibbonAction::ToggleFormatPainter => {
                if self.format_painter.is_some() {
                    self.format_painter = None;
                } else {
                    self.format_painter =
                        Some(self.square.formats.get(self.square.selection.cursor));
                }
            }
            RibbonAction::OpenHelp => self.help_open = true,
            RibbonAction::ToggleTheme => self.dark_mode = !self.dark_mode,
            RibbonAction::OpenFind => {
                self.find_open = true;
                self.find_just_opened = true;
            }
            RibbonAction::FindNext => self.find_step(true),
            RibbonAction::FindPrev => self.find_step(false),
            RibbonAction::SelectAll => self.select_all(),
            RibbonAction::SelectRegion => self.select_region(),
            RibbonAction::OpenNote => {
                let cell = self.square.selection.cursor;
                self.note_cell = CellId::Square(cell);
                self.note_draft = self.notes.get(cell).to_string();
                self.note_open = true;
            }
            RibbonAction::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            RibbonAction::SortAscending => self.sort_selection(true),
            RibbonAction::SortDescending => self.sort_selection(false),
            RibbonAction::ZoomIn => {
                let z = (ctx.zoom_factor() + 0.1).min(3.0);
                ctx.set_zoom_factor(z);
            }
            RibbonAction::ZoomOut => {
                let z = (ctx.zoom_factor() - 0.1).max(0.5);
                ctx.set_zoom_factor(z);
            }
            RibbonAction::ResetZoom => ctx.set_zoom_factor(1.0),
            RibbonAction::OpenAbout => self.about_open = true,
            RibbonAction::Save => self.handle_save(false),
            RibbonAction::Open => self.handle_open(),
            RibbonAction::ToggleVoronoiFreeze => {
                let sid = self.voronoi.sheet_id;
                let now = self.engine.voronoi_frozen(sid);
                let _ = self.engine.set_voronoi_frozen(sid, !now);
                self.mark_dirty();
            }
        }
    }

    /// Turn the selected square-sheet cells into boolean checkbox cells,
    /// or back into ordinary cells if they all already are. A no-op on
    /// the hex sheet, which has no widgets yet.
    fn toggle_widget_cells(&mut self) {
        if self.active != ActiveSheet::Square {
            return;
        }
        let cells = self.square.selection.primary_cells();
        let all_on = cells.iter().all(|&c| self.square_widgets.is_toggle(c));
        for cell in cells {
            self.square_widgets.set_toggle(cell, !all_on);
        }
    }

    /// Convert the selected square cells into slider widgets — or
    /// back into ordinary cells if they all already are. Sliders use
    /// the default 0–100 range; per-cell range config lands later.
    fn toggle_slider_cells(&mut self) {
        if self.active != ActiveSheet::Square {
            return;
        }
        let cells = self.square.selection.primary_cells();
        let all_on = cells.iter().all(|&c| self.square_widgets.is_slider(c));
        for cell in cells {
            self.square_widgets.set_slider_default(cell, !all_on);
        }
    }

    /// Convert the selected square cells into button widgets — or
    /// back into ordinary cells if they all already are.
    fn toggle_button_cells(&mut self) {
        if self.active != ActiveSheet::Square {
            return;
        }
        let cells = self.square.selection.primary_cells();
        let all_on = cells.iter().all(|&c| self.square_widgets.is_button(c));
        for cell in cells {
            self.square_widgets.set_button(cell, !all_on);
        }
    }

    /// Convert the selected square cells into progress-bar widgets
    /// (default 0–100 max) — or back into ordinary cells if they all
    /// already are.
    fn toggle_progress_bar_cells(&mut self) {
        if self.active != ActiveSheet::Square {
            return;
        }
        let cells = self.square.selection.primary_cells();
        let all_on = cells
            .iter()
            .all(|&c| self.square_widgets.is_progress_bar(c));
        for cell in cells {
            if all_on {
                self.square_widgets.set(cell, None);
            } else {
                self.square_widgets
                    .set_progress_bar(cell, widget::Widgets::<(u32, u32)>::DEFAULT_PROGRESS_MAX);
            }
        }
    }

    /// AutoSum — write `=FUNC(...)` of the selection (`func` is an
    /// aggregate like `"SUM"` or `"AVERAGE"`) into the cell directly
    /// below it, then move there. A no-op when that cell would fall off
    /// the sheet — past the last row, or outside the hex disc.
    fn autosum(&mut self, func: &str) {
        match self.active {
            ActiveSheet::Square => {
                let Some((target, formula)) =
                    grid::autosum(self.square.selection.bounds(), ROWS, func)
                else {
                    return;
                };
                self.commit_edit();
                let addr = grid::cell_address(target.0, target.1);
                self.apply_edits(self.square.sheet_id, vec![(addr, Some(formula))]);
                self.square.selection.collapse_to(target);
            }
            ActiveSheet::Hex => {
                let Some((target, formula)) = hex_autosum(self.hex.selection.bounds(), func) else {
                    return;
                };
                self.commit_edit();
                self.apply_edits(
                    self.hex.sheet_id,
                    vec![(hex_address(target), Some(formula))],
                );
                self.hex.selection.collapse_to(target);
            }
            ActiveSheet::Triangle => {
                let Some((target, formula)) =
                    triangle_autosum(self.triangle.selection.bounds(), func)
                else {
                    return;
                };
                self.commit_edit();
                self.apply_edits(
                    self.triangle.sheet_id,
                    vec![(triangle_address(target), Some(formula))],
                );
                self.triangle.selection.collapse_to(target);
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// Fill series — fill each selected column with an arithmetic
    /// progression extrapolated from the numbers already at the top of
    /// that column (a q-line, down the r-axis, on the hex sheet). A
    /// column with no numeric seed is left alone. One undo step.
    fn fill_series(&mut self) {
        match self.active {
            ActiveSheet::Square => {
                let ((min_c, min_r), (max_c, max_r)) = self.square.selection.bounds();
                let total = (max_r - min_r + 1) as usize;
                let mut targets = Vec::new();
                for c in min_c..=max_c {
                    let mut seed = Vec::new();
                    for r in min_r..=max_r {
                        match self.cell_value(c, r) {
                            CellValue::Number(n) => seed.push(n),
                            CellValue::Integer(i) => seed.push(i as f64),
                            _ => break,
                        }
                    }
                    if seed.is_empty() {
                        continue;
                    }
                    for (i, v) in grid::series_fill(&seed, total).into_iter().enumerate() {
                        let r = min_r + i as u32;
                        targets.push((grid::cell_address(c, r), Some(v.to_string())));
                    }
                }
                if !targets.is_empty() {
                    self.commit_edit();
                    self.apply_edits(self.square.sheet_id, targets);
                }
            }
            ActiveSheet::Hex => {
                let (min, max) = self.hex.selection.bounds();
                let (min_q, min_r, max_q, max_r) = (min.q, min.r, max.q, max.r);
                let total = (max_r - min_r + 1) as usize;
                let mut targets = Vec::new();
                for q in min_q..=max_q {
                    let mut seed = Vec::new();
                    for r in min_r..=max_r {
                        match self.hex_cell_value(HexCoord::new(q, r)) {
                            CellValue::Number(n) => seed.push(n),
                            CellValue::Integer(i) => seed.push(i as f64),
                            _ => break,
                        }
                    }
                    if seed.is_empty() {
                        continue;
                    }
                    for (i, v) in grid::series_fill(&seed, total).into_iter().enumerate() {
                        let r = min_r + i as i32;
                        targets.push((hex_address(HexCoord::new(q, r)), Some(v.to_string())));
                    }
                }
                if !targets.is_empty() {
                    self.commit_edit();
                    self.apply_edits(self.hex.sheet_id, targets);
                }
            }
            ActiveSheet::Triangle => {
                // Triangle series-fill lands in a follow-up.
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// Sort the selected block by the cursor's column, whole rows moving
    /// together — ascending, or descending when `ascending` is false. A
    /// stable sort: rows with equal keys keep their order. Cell sources
    /// move with their values. One undo step. On the hex sheet the
    /// cursor's q-line is the key, the rows running down the r-axis.
    fn sort_selection(&mut self, ascending: bool) {
        match self.active {
            ActiveSheet::Square => {
                let ((min_c, min_r), (max_c, max_r)) = self.square.selection.bounds();
                let key_col = self.square.selection.cursor.0;
                let keys: Vec<CellValue> = (min_r..=max_r)
                    .map(|r| self.cell_value(key_col, r))
                    .collect();
                let order = sort::row_order(&keys, ascending);
                let mut targets: Vec<(String, Option<String>)> = Vec::new();
                for c in min_c..=max_c {
                    let sources: Vec<String> =
                        (min_r..=max_r).map(|r| self.cell_source(c, r)).collect();
                    for (i, &src_row) in order.iter().enumerate() {
                        let r = min_r + i as u32;
                        let source = sources[src_row].clone();
                        let cell = (!source.is_empty()).then_some(source);
                        targets.push((grid::cell_address(c, r), cell));
                    }
                }
                self.commit_edit();
                self.apply_edits(self.square.sheet_id, targets);
            }
            ActiveSheet::Hex => {
                let (min, max) = self.hex.selection.bounds();
                let (min_q, min_r, max_q, max_r) = (min.q, min.r, max.q, max.r);
                let key_q = self.hex.selection.cursor.q;
                let keys: Vec<CellValue> = (min_r..=max_r)
                    .map(|r| self.hex_cell_value(HexCoord::new(key_q, r)))
                    .collect();
                let order = sort::row_order(&keys, ascending);
                let mut targets: Vec<(String, Option<String>)> = Vec::new();
                for q in min_q..=max_q {
                    let sources: Vec<String> = (min_r..=max_r)
                        .map(|r| self.hex_cell_source(HexCoord::new(q, r)))
                        .collect();
                    for (i, &src_row) in order.iter().enumerate() {
                        let r = min_r + i as i32;
                        let source = sources[src_row].clone();
                        let cell = (!source.is_empty()).then_some(source);
                        targets.push((hex_address(HexCoord::new(q, r)), cell));
                    }
                }
                self.commit_edit();
                self.apply_edits(self.hex.sheet_id, targets);
            }
            ActiveSheet::Triangle => {
                let (min, max) = self.triangle.selection.bounds();
                let (min_col, min_row, max_col, max_row) = (min.col, min.row, max.col, max.row);
                let key_col = self.triangle.selection.cursor.col;
                let keys: Vec<CellValue> = (min_row..=max_row)
                    .map(|r| self.triangle_cell_value(TriCoord::new(key_col, r)))
                    .collect();
                let order = sort::row_order(&keys, ascending);
                let mut targets: Vec<(String, Option<String>)> = Vec::new();
                for col in min_col..=max_col {
                    let sources: Vec<String> = (min_row..=max_row)
                        .map(|r| self.triangle_cell_source(TriCoord::new(col, r)))
                        .collect();
                    for (i, &src_row) in order.iter().enumerate() {
                        let row = min_row + i as i32;
                        let source = sources[src_row].clone();
                        let cell = (!source.is_empty()).then_some(source);
                        targets.push((triangle_address(TriCoord::new(col, row)), cell));
                    }
                }
                self.commit_edit();
                self.apply_edits(self.triangle.sheet_id, targets);
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// Apply a border mode across the selection — one undo step. Both
    /// sheets resolve borders per cell: the square sheet via
    /// `border_sides`, the hex sheet via `apply_hex_border`.
    fn apply_border(&mut self, mode: BorderMode) {
        match self.active {
            ActiveSheet::Square => self.apply_square_border(mode),
            ActiveSheet::Hex => self.apply_hex_border(mode),
            ActiveSheet::Triangle => {
                // Triangle borders land in a follow-up — triangles
                // have three edges, not four, and need their own
                // border-mode resolution.
            }
            ActiveSheet::Voronoi => {}
        }
    }

    /// The square-sheet border apply — `border_sides` per cell.
    fn apply_square_border(&mut self, mode: BorderMode) {
        let bounds = self.square.selection.bounds();
        let cells = self.square.selection.primary_cells();
        let mut edits = Vec::new();
        for cell in cells {
            let before = self.square.formats.get(cell);
            let mut after = before.clone();
            after.borders = format::border_sides(cell, bounds, mode);
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
        if !edits.is_empty() {
            self.history.record(Action::Formats(edits));
        }
    }

    /// The hex-sheet border apply. `All` and `None` set every cell's
    /// edges uniformly; `Outer` resolves each cell's perimeter via
    /// `hex_outer_borders` — an edge is bordered only when the hex
    /// across it is not itself selected.
    fn apply_hex_border(&mut self, mode: BorderMode) {
        let selection = self.hex.selection.clone();
        let mut edits = Vec::new();
        for cell in selection.primary_cells() {
            let before = self.hex.formats.get(cell);
            let mut after = before.clone();
            after.hex_borders = match mode {
                BorderMode::None => HexBorders::default(),
                BorderMode::All => HexBorders::all(),
                BorderMode::Outer => {
                    format::hex_outer_borders(cell, |c| selection.primary_contains(c))
                }
            };
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
        if !edits.is_empty() {
            self.history.record(Action::HexFormats(edits));
        }
    }

    /// Apply a formatting change to every cell of the active sheet's
    /// selection, recorded as one undo step. Cells the change leaves
    /// untouched are dropped.
    fn format_range(&mut self, edit: impl Fn(&mut CellFormat)) {
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
    fn toggle_range(
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
    fn move_active(&mut self, dir: Dir) {
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
    fn voronoi_advance(&mut self) {
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
    fn page(&mut self, up: bool) {
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
    fn jump_active(&mut self, dir: Dir) {
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
    fn jump_extend_active(&mut self, dir: Dir) {
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
    fn square_occupied(&self, col: u32, row: u32) -> bool {
        !matches!(self.cell_value(col, row), CellValue::Empty)
    }

    /// Whether a hex-sheet cell holds content — the predicate the hex
    /// Ctrl+arrow jump scans.
    fn hex_occupied(&self, coord: HexCoord) -> bool {
        !matches!(self.hex_cell_value(coord), CellValue::Empty)
    }

    /// Extend the selection one cell — Shift+arrow, on either sheet.
    fn extend_active(&mut self, dir: Dir) {
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

    fn begin_edit(&mut self, replace_with: Option<char>) {
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
            ActiveSheet::Voronoi => {}
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
    fn apply_edits(&mut self, sheet: SheetId, targets: Vec<(String, Option<String>)>) {
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

    /// Write the in-progress edit to the active sheet and leave editing
    /// mode. An all-whitespace buffer clears the cell.
    fn commit_edit(&mut self) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        self.square.formula_highlight = None;
        self.square.formula_drag = None;
        self.hex.formula_highlight = None;
        self.hex.formula_drag = None;
        let source = commit_source(&edit.buffer);
        let (sheet, addr) = self.active_target();
        self.apply_edits(sheet, vec![(addr, source)]);
    }

    /// Clear the selection on whichever sheet is active — the whole range
    /// on the square sheet, the single selected cell on the hex sheet.
    fn clear_active(&mut self) {
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
                let cursor = self.voronoi.selection.cursor;
                (self.voronoi.sheet_id, vec![(voronoi_address(cursor), None)])
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
    fn copy_selection(&mut self, ctx: &egui::Context) {
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
            ActiveSheet::Voronoi => {}
        }
    }

    /// Cut the selected range — captures it like a copy, then arms the
    /// range so the next paste clears it. Works on either sheet.
    fn cut_selection(&mut self, ctx: &egui::Context) {
        self.copy_selection(ctx);
        // copy_selection just rebuilt the clipboard as a copy — mark it a
        // cut, recording the leading corner of the selection's origin.
        let origin = match self.active {
            ActiveSheet::Square => {
                let ((c, r), _) = self.square.selection.bounds();
                (c as i32, r as i32)
            }
            ActiveSheet::Hex => {
                let (min, _) = self.hex.selection.bounds();
                (min.q, min.r)
            }
            ActiveSheet::Triangle => {
                let (min, _) = self.triangle.selection.bounds();
                (min.col, min.row)
            }
            // Voronoi has no grid origin — pick (0, 0) so the math
            // downstream keeps a sensible default; copy/cut on the
            // Voronoi sheet is a v151 follow-up anyway.
            ActiveSheet::Voronoi => (0, 0),
        };
        self.clipboard.mark_as_cut(origin);
    }

    /// Paste the clipboard block with its top-left at the active cell,
    /// recorded as one undo step. Pasting onto the same kind of sheet
    /// the copy came from writes the cell sources; pasting onto a
    /// different lattice writes each cell's value instead.
    fn paste(&mut self, mode: PasteMode) {
        if self.clipboard.is_empty() {
            return;
        }
        let (width, height) = self.clipboard.dimensions();
        match self.active {
            ActiveSheet::Square => {
                let (target_c, target_r) = self.square.selection.cursor;
                let mut targets = Vec::new();
                // A cut from this sheet clears its origin cells — queued
                // first, so a paste on the same cell overrides the clear.
                if let Some((oc, or)) = self.clipboard.cut_origin() {
                    if matches!(self.clipboard.source(), SourceLattice::Square) {
                        for j in 0..height {
                            for i in 0..width {
                                let addr = grid::cell_address(oc as u32 + i, or as u32 + j);
                                targets.push((addr, None));
                            }
                        }
                    }
                }
                for (rel_c, rel_r, cell) in self.clipboard.entries() {
                    let c = target_c + rel_c;
                    let r = target_r + rel_r;
                    if c >= COLS || r >= ROWS {
                        continue;
                    }
                    let source = self.clipboard.paste_text(cell, SourceLattice::Square, mode);
                    targets.push((grid::cell_address(c, r), source));
                }
                self.apply_edits(self.square.sheet_id, targets);
                // Select the pasted block, the active cell at its top-left.
                let end_c = (target_c + width - 1).min(COLS - 1);
                let end_r = (target_r + height - 1).min(ROWS - 1);
                self.square.selection =
                    SquareSelection::from_anchor_cursor((end_c, end_r), (target_c, target_r));
            }
            ActiveSheet::Hex => {
                let cursor = self.hex.selection.cursor;
                let mut targets = Vec::new();
                // A cut from this sheet clears its origin cells — queued
                // first, so a paste on the same cell overrides the clear.
                if let Some((oq, or)) = self.clipboard.cut_origin() {
                    if matches!(self.clipboard.source(), SourceLattice::Hex) {
                        for j in 0..height {
                            for i in 0..width {
                                let coord = HexCoord::new(oq + i as i32, or + j as i32);
                                targets.push((hex_address(coord), None));
                            }
                        }
                    }
                }
                for (rel_c, rel_r, cell) in self.clipboard.entries() {
                    let coord = HexCoord::new(cursor.q + rel_c as i32, cursor.r + rel_r as i32);
                    if !hex_in_view(coord) {
                        continue;
                    }
                    let source = self.clipboard.paste_text(cell, SourceLattice::Hex, mode);
                    targets.push((hex_address(coord), source));
                }
                self.apply_edits(self.hex.sheet_id, targets);
                // Select the pasted block, the cursor at its origin.
                let far = HexCoord::new(cursor.q + width as i32 - 1, cursor.r + height as i32 - 1);
                self.hex.selection = HexSelection::from_anchor_cursor(far, cursor);
            }
            ActiveSheet::Triangle => {
                let cursor = self.triangle.selection.cursor;
                let mut targets = Vec::new();
                // A cut from this sheet clears its origin cells.
                if let Some((oc, or)) = self.clipboard.cut_origin() {
                    if matches!(self.clipboard.source(), SourceLattice::Triangle) {
                        for j in 0..height {
                            for i in 0..width {
                                let coord = TriCoord::new(oc + i as i32, or + j as i32);
                                targets.push((triangle_address(coord), None));
                            }
                        }
                    }
                }
                for (rel_c, rel_r, cell) in self.clipboard.entries() {
                    let coord = TriCoord::new(cursor.col + rel_c as i32, cursor.row + rel_r as i32);
                    if !triangle_in_view(coord) {
                        continue;
                    }
                    let source = self
                        .clipboard
                        .paste_text(cell, SourceLattice::Triangle, mode);
                    targets.push((triangle_address(coord), source));
                }
                self.apply_edits(self.triangle.sheet_id, targets);
                let far = TriCoord::new(
                    cursor.col + width as i32 - 1,
                    cursor.row + height as i32 - 1,
                );
                self.triangle.selection = TriangleSelection::from_anchor_cursor(far, cursor);
            }
            ActiveSheet::Voronoi => {}
        }
        // A paste consumes the cut — the clipboard reverts to a copy.
        self.clipboard.consume_cut();
    }

    /// Revert the most recent action — cell content or formatting. The
    /// engine recomputes any dependents.
    fn undo(&mut self) {
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
                    self.square
                        .formats
                        .update(edit.cell, |f| *f = edit.before.clone());
                }
            }
            Action::HexFormats(edits) => {
                for edit in &edits {
                    self.hex
                        .formats
                        .update(edit.cell, |f| *f = edit.before.clone());
                }
            }
        }
    }

    /// Re-apply the most recently undone action.
    fn redo(&mut self) {
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
                    self.square
                        .formats
                        .update(edit.cell, |f| *f = edit.after.clone());
                }
            }
            Action::HexFormats(edits) => {
                for edit in &edits {
                    self.hex
                        .formats
                        .update(edit.cell, |f| *f = edit.after.clone());
                }
            }
        }
    }

    /// Fill the selection's leading edge across the rest of it — Ctrl+D
    /// (down) / Ctrl+R (right), on either sheet. Sources copy verbatim
    /// and the whole fill is a single undo step.
    fn fill(&mut self, dir: FillDir) {
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
    fn fill_handle_visual_rect(&self, origin: egui::Pos2) -> egui::Rect {
        const SIZE: f32 = 8.0;
        let (_, (max_c, max_r)) = self.square.selection.bounds();
        let cell = self.metrics.cell_rect(origin, max_c, max_r);
        let center = cell.right_bottom();
        egui::Rect::from_center_size(center, egui::vec2(SIZE, SIZE))
    }

    /// The generous hit-test zone around the fill handle — visibly the
    /// same corner, but with a wider catchment so near-misses still
    /// register. Matches the way Excel widens the handle hit area.
    fn fill_handle_hit_rect(&self, origin: egui::Pos2) -> egui::Rect {
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
    fn fill_handle_apply(
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
    fn handle_resize(
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
    fn autofit_column(&mut self, col: u32, painter: &egui::Painter) {
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
    fn autofit_row(&mut self, row: u32, painter: &egui::Painter) {
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
    fn cell_under(
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

impl eframe::App for CarbideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        for command in self.collect_commands(ctx) {
            // Every dispatched command is a potential mutation; the
            // central mark_dirty here means we don't have to thread
            // mark_dirty through each command's body. Save/Open/Find/
            // help/etc. flip the bit too but the maybe_autosave debounce
            // makes that a wash — at worst a redundant write 2s later.
            // Save and Open clear the bit themselves on success.
            self.mark_dirty();
            self.apply(command, ctx);
        }

        // Menu bar above the ribbon — File / Edit / Format / Data /
        // View / Help. Items dispatch through the same `RibbonAction`
        // pipeline so the ribbon and menu share handlers.
        //
        // Stage Mode hides every editing-chrome panel — menu bar,
        // ribbon, formula bar, sheet tabs — so a sheet reads as an
        // app rather than a spreadsheet.
        if !self.stage_mode {
            egui::TopBottomPanel::top("carbide_menu_bar").show(ctx, |ui| {
                let can_undo = self.history.can_undo();
                let can_redo = self.history.can_redo();
                if let Some(action) = ribbon::menu_bar(ui, can_undo, can_redo) {
                    self.apply_ribbon(action, ctx);
                }
            });
        }

        if !self.stage_mode {
            egui::TopBottomPanel::top("carbide_ribbon").show(ctx, |ui| match self.active {
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
                        self.apply_ribbon(action, ctx);
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
                        self.apply_ribbon(action, ctx);
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
                        self.apply_ribbon(action, ctx);
                    }
                }
                ActiveSheet::Voronoi => {
                    // T-007: contextual Voronoi ribbon group surfaces only here.
                    let current = CellFormat::default();
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
                        self.apply_ribbon(action, ctx);
                    }
                }
            });
        }

        if !self.stage_mode {
            egui::TopBottomPanel::top("carbide_formula_bar")
                .resizable(true)
                .min_height(28.0)
                .show(ctx, |ui| {
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
                            ActiveSheet::Voronoi if !self.voronoi.selection.extra.is_empty() => {
                                // Marquee dimension hint: extras count + 1 (the cursor).
                                let n = self.voronoi.selection.extra.len() + 1;
                                ui.label(egui::RichText::new(format!("{n} cells")).weak());
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
            egui::TopBottomPanel::bottom("carbide_sheet_tabs").show(ctx, |ui| {
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
            let exit_area = egui::Area::new("carbide_stage_exit".into())
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
                .interactable(true);
            exit_area.show(ctx, |ui| {
                if ui
                    .add(egui::Button::new("Exit Stage Mode").min_size(egui::vec2(130.0, 28.0)))
                    .clicked()
                {
                    self.stage_mode = false;
                }
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.active {
            ActiveSheet::Square => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.draw_grid(ui));
            }
            ActiveSheet::Hex => self.draw_hex_grid(ui),
            ActiveSheet::Triangle => self.draw_triangle_grid(ui),
            ActiveSheet::Voronoi => self.draw_voronoi_grid(ui),
        });

        self.conditional_window(ctx);
        self.find_window(ctx);
        self.help_window(ctx);
        self.about_window(ctx);
        self.note_window(ctx);

        // End of frame — try a debounced autosave. The debounce in
        // `maybe_autosave` means a flurry of edits still results in at
        // most one write every 2 s.
        self.maybe_autosave(self.frame_time);
    }
}
