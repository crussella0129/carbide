//! Keyboard command collection/dispatch, ribbon actions, sorting, borders, and widgets.

use carbide_core::CellValue;
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use eframe::egui;

use crate::clipboard::PasteMode;
use crate::format::{self, BorderMode, CellFormat, HexBorders};
use crate::grid;
use crate::keymap::{self, Command, Mode};
use crate::ribbon::RibbonAction;
use crate::selection::{FillDir, HexSelection, Selection, TriangleSelection};
use crate::sort;
use crate::widget::{self, WidgetKind};

use super::helpers::{
    hex_address, hex_autosum, hex_current_region, hex_in_view, toggle_widget_kind,
    triangle_address, triangle_autosum,
};
use super::types::{Action, ActiveSheet, CellId, FormatEdit};
use super::{CarbideApp, COLS, EDIT_KEYS, HEX_VIEW_RADIUS, NAV_KEYS, ROWS, TRIANGLE_RADIUS};

impl CarbideApp {
    /// Read key events and turn them into commands through `keymap`. Keys
    /// that map to a command are consumed so no other widget reacts too.
    pub(super) fn collect_commands(&self, ctx: &egui::Context) -> Vec<Command> {
        // A focused text input that is not the in-cell editor — the
        // formula bar, or a dialog field — owns the keyboard; don't also
        // read its keystrokes as cell commands.
        if self.edit.is_none() && ctx.egui_wants_keyboard_input() {
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

    pub(super) fn apply(&mut self, command: Command, ctx: &egui::Context) {
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
                self.clear_formula_refs();
            }
            Command::ClearMarquee => {
                // Escape exits Stage Mode first — that's the most
                // load-bearing thing Escape can do when the editing
                // chrome is hidden.
                if self.stage_mode {
                    self.stage_mode = false;
                    return;
                }
                if self.clipboard.has_cut() {
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
    pub(super) fn select_region(&mut self) {
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
    pub(super) fn apply_ribbon(&mut self, action: RibbonAction, ctx: &egui::Context) {
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
                let cell = self.active_cell_id();
                self.open_note(cell);
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
            RibbonAction::Import => self.handle_import(ctx),
            RibbonAction::ToggleVoronoiFreeze => {
                let sid = self.voronoi.sheet_id;
                let now = self.engine.voronoi_frozen(sid);
                let _ = self.engine.set_voronoi_frozen(sid, !now);
                self.mark_dirty();
            }
        }
    }

    /// Toggle `kind` (Toggle or Button) across the active sheet's selection
    /// on any lattice — using `selection.cells()` on Voronoi so marquee
    /// `Selection.extra` cells are included.
    fn toggle_multi_lattice_widget(&mut self, kind: WidgetKind) {
        match self.active {
            ActiveSheet::Square => {
                let cells = self.square.selection.primary_cells();
                toggle_widget_kind(&mut self.square_widgets, &cells, kind);
            }
            ActiveSheet::Hex => {
                let cells = self.hex.selection.primary_cells();
                toggle_widget_kind(&mut self.hex_widgets, &cells, kind);
            }
            ActiveSheet::Triangle => {
                let cells = self.triangle.selection.primary_cells();
                toggle_widget_kind(&mut self.triangle_widgets, &cells, kind);
            }
            ActiveSheet::Voronoi => {
                let cells = self.voronoi.selection.cells();
                toggle_widget_kind(&mut self.voronoi_widgets, &cells, kind);
            }
        }
    }

    /// Turn the selected cells on the active sheet into boolean checkbox
    /// cells, or back into ordinary cells if they all already are.
    pub(super) fn toggle_widget_cells(&mut self) {
        self.toggle_multi_lattice_widget(WidgetKind::Toggle);
    }

    /// Convert the selected square cells into slider widgets — or
    /// back into ordinary cells if they all already are. Sliders use
    /// the default 0–100 range and remain square-only (ADR-006).
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

    /// Convert the selected cells on the active sheet into button widgets
    /// — or back into ordinary cells if they all already are.
    fn toggle_button_cells(&mut self) {
        self.toggle_multi_lattice_widget(WidgetKind::Button);
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
    pub(super) fn fill_series(&mut self) {
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
    pub(super) fn sort_selection(&mut self, ascending: bool) {
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
                cell: CellId::Square(cell),
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
            edits.push(FormatEdit {
                cell: CellId::Hex(cell),
                before,
                after,
            });
        }
        if !edits.is_empty() {
            self.history.record(Action::Formats(edits));
        }
    }
}
