//! Workbook bootstrap and built-in demo sheet seeding (`CarbideApp::new`).

use carbide_formula::WorkbookEngine;
use carbide_tess::hex::{HexCoord, HexLattice};
use carbide_tess::triangle::{TriCoord, TriangleLattice};
use carbide_tess::voronoi::{VoronoiCoord, VoronoiLattice};
use carbide_tess::LatticeKind;

use crate::clipboard::Clipboard;
use crate::conditional::{Condition, Rule};
use crate::find::FindState;
use crate::format::{CellFormat, FormatMap};
use crate::grid::GridMetrics;
use crate::history::History;
use crate::note::NoteMap;
use crate::selection::{HexSelection, Selection, Sheet};
use crate::widget::{self, Widgets};

use super::types::{ActiveSheet, CellId, CondDraft};
use super::{CarbideApp, HEX_SIZE, TRIANGLE_SIDE};

impl CarbideApp {
    /// Build the app, seeding a square demo sheet and a hex demo sheet.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut engine = WorkbookEngine::new();
        engine.new_workbook();

        let square_sheet = engine.add_sheet("Budget", LatticeKind::Square);
        // Real-Time Budget demo (launch Demo A). Slider-driven spend
        // categories feed a Total cell whose value the engine
        // recomputes through the DAG on every slider tick — drag any
        // slider and the Total and Remaining cells update in step.
        for (addr, src) in [
            ("A1", "Monthly Budget"),
            ("A3", "Income"),
            ("B3", "=4000"),
            ("A5", "Rent"),
            ("B5", "=1200"),
            ("A6", "Food"),
            ("B6", "=450"),
            ("A7", "Transport"),
            ("B7", "=180"),
            ("A8", "Savings Goal"),
            ("B8", "=500"),
            ("A10", "Total Spend"),
            ("B10", "=B5+B6+B7+B8"),
            ("A11", "Remaining"),
            ("B11", "=B3-B10"),
            ("A12", "Status"),
            ("B12", "=IF(B11>=0, \"Under\", \"Over\")"),
            ("A13", "Goal Progress"),
            // Cell mirrors Remaining so the progress bar fills as the
            // user under-spends. The widget's max is the initial
            // Savings Goal (500); changing the goal at runtime won't
            // re-tune the bar — that lands when widget config grows
            // a formula-bound max.
            ("B13", "=B11"),
        ] {
            let _ = engine.set_cell(square_sheet, addr, Some(src));
        }

        // Hex Game demo (launch Demo B). A pointy-top hex board with
        // resource tiles around the centre and a harvest cell that
        // sums the six edge-neighbours via `NEIGHBORS()`. Drag a tile
        // to edit its value; the harvest cell updates through the DAG
        // in step. The dice-button + counter half of the demo seeds
        // a Roll Dice button at H(2,2) (source `=RANDBETWEEN(1,6)`,
        // rerolls on every click) and a Score readout at H(3,2) that
        // mirrors the dice value.
        let hex_sheet = engine.add_sheet("Hex Game", LatticeKind::HexPointy);
        for (addr, src) in [
            // The center: a label, plus a Harvest cell directly below
            // it that aggregates the six edge-neighbours via NEIGHBORS.
            ("H(0,0)", "Center"),
            ("H(0,2)", "=SUM(NEIGHBORS(H(0,0)))"),
            // Inner ring — six resource tiles (wood / ore / grain
            // round-robined) with numeric yields.
            ("H(1,0)", "=4"),  // wood
            ("H(-1,0)", "=3"), // ore
            ("H(0,1)", "=5"),  // grain
            ("H(0,-1)", "=6"), // wood
            ("H(1,-1)", "=2"), // ore
            ("H(-1,1)", "=7"), // grain
            // Roll Dice — the cell's source IS the button's label;
            // clicking re-fires the source through the engine, which
            // for RANDBETWEEN advances the PRNG.
            ("H(2,2)", "=RANDBETWEEN(1,6)"),
            // Score readout — mirrors the dice value so a dedicated
            // cell shows the most recent roll in plain form.
            ("H(3,2)", "=H(2,2)"),
        ] {
            let _ = engine.set_cell(hex_sheet, addr, Some(src));
        }

        let mut hex_formats = FormatMap::new();
        // A seeded demo so hex formatting is visible at launch.
        hex_formats.update(HexCoord::new(1, 0), |f| {
            f.fill = Some(egui::Color32::from_rgb(201, 237, 203));
        });

        // A small triangle demo sheet — a row of alternating △ / ▽
        // triangles plus one cell that sums two of them, so the
        // engine's triangle range arithmetic is visible.
        let triangle_sheet = engine.add_sheet("Tri demo", LatticeKind::Triangle);
        for (addr, src) in [
            // egui's default font doesn't carry the geometric-shape
            // glyphs `△ ▽`, which render as tofu boxes — using short
            // text labels makes the orientation legible without a
            // custom font. Loading a Unicode font with these glyphs
            // (or rendering the orientation indicator as a small
            // geometric drawing in `draw_triangle_grid`) is a clean
            // follow-up.
            ("T(0,0)", "up"),
            ("T(1,0)", "dn"),
            ("T(2,0)", "=4"),
            ("T(3,0)", "=6"),
            ("T(2,1)", "=T(2,0) + T(3,0)"),
            // Toggle widget seed (paired with the triangle_widgets
            // initialiser above). Starts unchecked so the user sees
            // the canonical "blank checkbox" state on first launch.
            ("T(2,-1)", "FALSE"),
        ] {
            let _ = engine.set_cell(triangle_sheet, addr, Some(src));
        }
        let mut triangle_formats = FormatMap::new();
        // Tint a couple of cells so the sheet visibly carries cell
        // formatting at first launch.
        triangle_formats.update(TriCoord::new(0, 0), |f| {
            f.fill = Some(egui::Color32::from_rgb(231, 217, 245));
        });
        triangle_formats.update(TriCoord::new(2, 1), |f| {
            f.fill = Some(egui::Color32::from_rgb(212, 233, 251));
        });

        let square = Sheet {
            sheet_id: square_sheet,
            selection: Selection::single((0, 0)),
            formula_drag: None,
            formula_highlight: None,
            formats: FormatMap::new(),
        };
        let hex = Sheet {
            sheet_id: hex_sheet,
            selection: HexSelection::single(HexCoord::new(0, 0)),
            formula_drag: None,
            formula_highlight: None,
            formats: hex_formats,
        };
        let triangle = Sheet {
            sheet_id: triangle_sheet,
            selection: Selection::single(TriCoord::new(0, 0)),
            formula_drag: None,
            formula_highlight: None,
            formats: triangle_formats,
        };

        // Voronoi sheet — Demo C. Uses the default 8-seed config from
        // `carbide-tess`. Seed cells populate `V(0)..V(7)` with
        // simple labels and one formula example.
        let voronoi_sheet = engine.add_sheet("Voronoi", LatticeKind::Voronoi);
        for (addr, src) in [
            ("V(0)", "Plains"),
            ("V(1)", "Forest"),
            ("V(2)", "=42"),
            ("V(3)", "Tundra"),
            ("V(4)", "=V(2) + 8"),
            // V(5) hosts a Toggle widget (seeded below) — start it
            // unchecked so the checkbox renders in its canonical state.
            ("V(5)", "FALSE"),
            ("V(6)", "Coast"),
            ("V(7)", "Highlands"),
        ] {
            let _ = engine.set_cell(voronoi_sheet, addr, Some(src));
        }
        let voronoi = Sheet {
            sheet_id: voronoi_sheet,
            selection: Selection::single(VoronoiCoord(0)),
            formula_drag: None,
            formula_highlight: None,
            formats: FormatMap::new(),
        };

        let mut this = Self {
            engine,
            square,
            hex,
            triangle,
            voronoi,
            hex_lattice: HexLattice::pointy(HEX_SIZE),
            triangle_lattice: TriangleLattice::new(TRIANGLE_SIDE),
            voronoi_lattice: VoronoiLattice::default(),
            voronoi_marquee_start: None,
            voronoi_visit_history: std::collections::VecDeque::new(),
            active: ActiveSheet::Square,
            prev_cursor: (0, 0),
            edit: None,
            metrics: GridMetrics::new(),
            resizing: None,
            press_pos: None,
            header_drag: None,
            format_painter: None,
            fill_drag: None,
            clipboard: Clipboard::default(),
            history: History::new(),
            frame_time: 0.0,
            last_format_time: 0.0,
            last_format_cells: Vec::new(),
            last_hex_format_time: 0.0,
            last_hex_format_cells: Vec::new(),
            cond_rules: vec![Rule {
                condition: Condition::GreaterThan(1000.0),
                format: CellFormat {
                    fill: Some(egui::Color32::from_rgb(201, 237, 203)),
                    ..CellFormat::default()
                },
            }],
            cond_window_open: false,
            cond_draft: CondDraft::default(),
            square_widgets: {
                let mut w: Widgets<(u32, u32)> = Widgets::default();
                w.set_toggle((3, 1), true);
                // Budget-demo sliders on B5..B8 (Rent / Food /
                // Transport / Savings Goal). 0–3000 range per the
                // launch brief; the cell sources above sit inside
                // this range so the thumb starts at each line item's
                // current value.
                for r in 4..=7 {
                    w.set_slider((1, r), 0.0, 3000.0);
                }
                // Goal Progress bar at B13 — max matches the initial
                // Savings Goal so the bar fills as the user
                // under-spends. Read-only.
                w.set_progress_bar((1, 12), 500.0);
                w
            },
            hex_widgets: {
                let mut w: Widgets<HexCoord> = Widgets::default();
                // Hex Game demo (Demo B completion): a Roll Dice
                // Button at H(2,2) whose source `=RANDBETWEEN(1,6)`
                // re-fires on every click (RANDBETWEEN advances the
                // PRNG each eval). Pair the dice cell with a
                // separate Score readout cell that mirrors the dice
                // value — seeded by T-201e's set_cell calls below.
                w.set_button(HexCoord::new(2, 2), true);
                w
            },
            triangle_widgets: {
                let mut w: Widgets<TriCoord> = Widgets::default();
                // A small Toggle seed at T(2, -1) — outside the
                // existing up/dn label pattern — so the triangle
                // sheet has a visible widget surface to point at in
                // docs/screenshots. The accompanying set_cell below
                // initialises the cell to FALSE.
                w.set_toggle(TriCoord::new(2, -1), true);
                w
            },
            voronoi_widgets: {
                let mut w: Widgets<VoronoiCoord> = Widgets::default();
                // Toggle on V(5) — gives the Voronoi sheet a visible
                // widget surface and closes the four-lattice symmetry.
                // The V(5) cell is seeded "FALSE" above.
                w.set_toggle(VoronoiCoord(5), true);
                w
            },
            formula_bar: String::new(),
            find: FindState::default(),
            find_open: false,
            find_just_opened: false,
            help_open: false,
            about_open: false,
            notes: NoteMap::new(),
            hex_notes: NoteMap::new(),
            triangle_notes: NoteMap::new(),
            note_open: false,
            note_cell: CellId::Square((0, 0)),
            note_draft: String::new(),
            name_box: String::new(),
            dark_mode: true,
            stage_mode: false,
            pending_open_bytes: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dirty: false,
            last_autosave: 0.0,
            suppress_autosave_until: 0.0,
        };

        // Widget-fit minimums: cells that host a slider, button, or
        // progress bar render their affordance inside the cell rect.
        // At the default 64 px column width an egui slider thumb + the
        // numeric value display look crammed; bump every widget column
        // to a friendlier minimum so the launch demo reads cleanly.
        // The user can resize columns wider after this initial pass —
        // we only enforce a *minimum*, never override a wider setting.
        let mut widget_cols: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let mut widget_rows: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for (cell, kind) in this.square_widgets.iter() {
            let (c, r) = *cell;
            match kind {
                widget::WidgetKind::Slider { .. } | widget::WidgetKind::ProgressBar { .. } => {
                    widget_cols.insert(c);
                }
                widget::WidgetKind::Button => {
                    widget_cols.insert(c);
                }
                widget::WidgetKind::Toggle => {} // a checkbox fits in the default cell
            }
            widget_rows.insert(r);
        }
        for col in widget_cols {
            let current = this.metrics.col_width(col);
            if current < 160.0 {
                this.metrics.set_col_width(col, 160.0);
            }
        }
        for row in widget_rows {
            let current = this.metrics.row_height(row);
            if current < 28.0 {
                this.metrics.set_row_height(row, 28.0);
            }
        }

        // Boot rehydrate: if an autosave is present, swap the seed-demo
        // workbook for the saved one. Failure here is silent — the seed
        // demos remain. Skips entirely on native (localStorage is wasm-only).
        if let Some(bytes) = crate::state_io::load_from_local_storage() {
            match this.engine.open_bytes(&bytes) {
                Ok(ui) => {
                    this.rebind_sheet_ids();
                    let snap = crate::state_io::ui_state_to_snapshot(&ui);
                    this.restore_state(snap);
                    this.history.clear();
                }
                Err(e) => eprintln!("boot: autosave rehydrate failed: {e:?}"),
            }
        }

        this
    }
}
