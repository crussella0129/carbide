//! Persistence I/O — `.crbd`/`.tscl` save/open dialogs, CSV/XLSX import,
//! wasm localStorage autosave, sheet ID rebinding, and `UiSnapshot` capture/restore.

use carbide_core::SheetId;
use carbide_formula::WorkbookEngine;
use carbide_store::import::{
    detect_import_format, import_csv_bytes, import_xlsx_bytes, sheet_name_from_filename,
    ImportFormat,
};
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use carbide_tess::voronoi::VoronoiCoord;
use carbide_tess::LatticeKind;
use eframe::egui;

use crate::format::FormatMap;
use crate::grid::GridMetrics;
use crate::note::NoteMap;
use crate::widget;

use super::types::{ActiveSheet, CellId};
use super::voronoi_helpers::synced_voronoi_lattice;
use super::CarbideApp;

/// Staged file payload from an Import dialog, drained on the next frame.
pub(super) struct PendingImport {
    pub filename: Option<String>,
    pub bytes: Vec<u8>,
}

/// Shared slot carrying a staged CSV/XLSX import payload across sync/async boundaries.
pub(super) type PendingImportSlot = std::sync::Arc<std::sync::Mutex<Option<PendingImport>>>;

/// Metadata returned when [`apply_imported_bytes`] replaces a sheet from
/// imported CSV or `.xlsx` bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImportOutcome {
    pub sheet_name: String,
    pub total_sheets: usize,
}

/// Pure/engine-level helper that parses CSV or `.xlsx` bytes and replaces
/// `target_sheet` in `engine` with the first imported sheet while leaving
/// sibling sheets (Hex, Triangle, Voronoi) completely untouched.
pub(super) fn apply_imported_bytes(
    engine: &mut WorkbookEngine,
    target_sheet: SheetId,
    filename: Option<&str>,
    bytes: &[u8],
) -> Result<ImportOutcome, String> {
    let format = detect_import_format(filename, bytes);
    let imported = match format {
        ImportFormat::Csv => {
            let name = sheet_name_from_filename(filename);
            import_csv_bytes(bytes, &name).map_err(|e| format!("CSV import failed: {e}"))?
        }
        ImportFormat::Xlsx => {
            import_xlsx_bytes(bytes).map_err(|e| format!("Excel (.xlsx) import failed: {e}"))?
        }
    };
    let total_sheets = imported.sheet_order.len();
    let first_sid = imported
        .sheet_order
        .first()
        .copied()
        .ok_or_else(|| "Imported workbook contains no sheets".to_string())?;
    let sheet = imported
        .sheets
        .get(&first_sid)
        .ok_or_else(|| "Imported sheet data missing".to_string())?;
    let sheet_name = sheet.name.clone();
    let cells = sheet.cells.clone();
    engine
        .replace_sheet_cells(target_sheet, Some(sheet_name.clone()), cells)
        .map_err(|e| format!("Failed to apply imported sheet: {e}"))?;
    Ok(ImportOutcome {
        sheet_name,
        total_sheets,
    })
}

impl CarbideApp {
    /// Flip the dirty flag. Called from every entry point that mutates
    /// engine or UI state — keeps the autosave wiring out of each call
    /// site and lets future mutation paths opt in with one line.
    pub(super) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// If the dirty-debounce has elapsed, persist the workbook + snapshot
    /// to localStorage and clear the flag. Called once per frame.
    pub(super) fn maybe_autosave(&mut self, now: f64) {
        if !self.dirty {
            return;
        }
        if now < self.suppress_autosave_until {
            return;
        }
        if now - self.last_autosave < 2.0 {
            return;
        }
        let snap = self.capture_state();
        let ui = crate::state_io::snapshot_to_ui_state(&snap);
        match self.engine.save_bytes(&ui) {
            Ok(bytes) => {
                crate::state_io::autosave_to_local_storage(&bytes);
                self.dirty = false;
                self.last_autosave = now;
            }
            Err(e) => eprintln!("autosave: save_bytes failed: {e:?}"),
        }
    }

    /// Persist the workbook + UI state to a `.crbd`. On native, opens a
    /// blocking save dialog; on wasm, spawns an async dialog that hands
    /// the bytes to the browser via a download blob. Errors are logged
    /// rather than panicked so a cancelled dialog or write failure
    /// doesn't take the app down. `_force_dialog` is unused today but
    /// is preserved for a future "remember last path" tweak.
    pub(super) fn handle_save(&mut self, _force_dialog: bool) {
        let snap = self.capture_state();
        let ui = crate::state_io::snapshot_to_ui_state(&snap);
        let bytes = match self.engine.save_bytes(&ui) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("save: engine.save_bytes failed: {e:?}");
                return;
            }
        };
        // We've serialized the current state — the autosave path can
        // skip a redundant write until the next mutation.
        self.dirty = false;
        self.last_autosave = self.frame_time;

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("workbook.crbd")
                .add_filter("Carbide", &["crbd"])
                .save_file()
            {
                if let Err(e) = std::fs::write(&path, &bytes) {
                    eprintln!("save: write to {} failed: {e:?}", path.display());
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_file_name("workbook.crbd")
                    .add_filter("Carbide", &["crbd"])
                    .save_file()
                    .await
                {
                    let _ = handle.write(&bytes).await;
                }
            });
        }
    }

    /// Load a workbook + UI state from a `.crbd`. On native, opens a
    /// blocking pick dialog and reads synchronously; on wasm, spawns the
    /// async picker and writes the resulting bytes into
    /// [`Self::pending_open_bytes`] for `update()` to drain on the next
    /// frame. Native could also use the slot for symmetry, but the
    /// synchronous path is simpler when it's available.
    pub(super) fn handle_open(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Carbide", &["crbd", "tscl"])
                .pick_file()
            {
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        if let Ok(mut slot) = self.pending_open_bytes.lock() {
                            *slot = Some(bytes);
                        }
                    }
                    Err(e) => eprintln!("open: read of {} failed: {e:?}", path.display()),
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let slot = self.pending_open_bytes.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("Carbide", &["crbd", "tscl"])
                    .pick_file()
                    .await
                {
                    let bytes = handle.read().await;
                    if let Ok(mut slot) = slot.lock() {
                        *slot = Some(bytes);
                    }
                }
            });
        }
    }

    /// Drain a pending Open: if `pending_open_bytes` carries bytes, load
    /// them into the engine and restore the snapshot. Errors are logged
    /// and non-fatal — a corrupt file leaves the current workbook in
    /// place rather than dropping the user into an empty workbook.
    pub(super) fn drain_pending_open(&mut self) {
        let bytes = self
            .pending_open_bytes
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let Some(bytes) = bytes else {
            return;
        };
        match self.engine.open_bytes(&bytes) {
            Ok(ui) => {
                // The engine just swapped in a fresh workbook with its own
                // SheetIds — re-bind our per-sheet UI bundles to whichever
                // saved sheet matches each lattice. Using sheet_order
                // gives stable per-lattice picking even when a workbook
                // carries multiple sheets of the same lattice.
                self.rebind_sheet_ids();
                let snap = crate::state_io::ui_state_to_snapshot(&ui);
                self.restore_state(snap);
                // Also reset transient state that pointed into the old
                // workbook — selection cursors stay at A1, edit/clipboard
                // start fresh.
                self.edit = None;
                self.history.clear();
                self.import_error = None;
                self.import_notice = None;
                // Don't immediately autosave the just-loaded state.
                self.dirty = false;
                self.last_autosave = self.frame_time;
                self.suppress_autosave_until = self.frame_time + 2.0;
            }
            Err(e) => eprintln!("open: engine.open_bytes failed: {e:?}"),
        }
    }

    /// Open a `.csv` or `.xlsx` file picker and stage the selected bytes for
    /// [`Self::drain_pending_import`] to apply on the next frame.
    pub(super) fn handle_import(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Spreadsheets (CSV, Excel)", &["csv", "xlsx"])
                .pick_file()
            {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string);
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        if let Ok(mut slot) = self.pending_import.lock() {
                            *slot = Some(PendingImport {
                                filename: file_name,
                                bytes,
                            });
                        }
                    }
                    Err(e) => {
                        self.import_notice = None;
                        self.import_error = Some(format!(
                            "Import failed — could not read {}: {e}",
                            path.display()
                        ));
                    }
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let slot = self.pending_import.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("Spreadsheets (CSV, Excel)", &["csv", "xlsx"])
                    .pick_file()
                    .await
                {
                    let name = handle.file_name();
                    let bytes = handle.read().await;
                    if let Ok(mut guard) = slot.lock() {
                        *guard = Some(PendingImport {
                            filename: Some(name),
                            bytes,
                        });
                    }
                    ctx.request_repaint();
                }
            });
        }
    }

    /// Drain pending CSV/XLSX import bytes, replacing the Square sheet's cells
    /// while preserving non-square tabs (Hex, Triangle, Voronoi) intact.
    pub(super) fn drain_pending_import(&mut self) {
        let pending = self
            .pending_import
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let Some(PendingImport { filename, bytes }) = pending else {
            return;
        };
        match apply_imported_bytes(
            &mut self.engine,
            self.square.sheet_id,
            filename.as_deref(),
            &bytes,
        ) {
            Ok(outcome) => {
                self.active = ActiveSheet::Square;
                // Clear all stale square-sheet and transient UI state so
                // old widgets, notes, formatting, sizing, undo history, or
                // cut/formula highlights cannot leak onto imported data.
                self.square.formats = FormatMap::new();
                self.square_widgets = widget::Widgets::default();
                self.notes = NoteMap::new();
                self.metrics = GridMetrics::new();
                self.square.selection.collapse_to((0, 0));
                self.prev_cursor = (0, 0);
                self.edit = None;
                self.history.clear();
                self.format_painter = None;
                self.square.formula_drag = None;
                self.square.formula_highlight = None;
                if matches!(self.note_cell, CellId::Square(_)) {
                    self.note_open = false;
                }
                self.clipboard.consume_cut();
                if self.find_open {
                    self.refresh_find();
                }
                self.import_error = None;
                self.import_notice = (outcome.total_sheets > 1).then(|| {
                    format!(
                        "Imported '{}' (sheet 1 of {})",
                        outcome.sheet_name, outcome.total_sheets
                    )
                });
                self.mark_dirty();
            }
            Err(msg) => {
                self.import_notice = None;
                self.import_error = Some(msg);
            }
        }
    }

    /// After an engine swap, re-bind the UI sheet bundles to whichever
    /// sheet in the new workbook matches each lattice. Picks the first
    /// matching sheet in `workbook.sheet_order` per lattice. Sheets the
    /// new workbook lacks keep their old IDs (stale; harmless since the
    /// user can't reach them through the tab bar).
    pub(super) fn rebind_sheet_ids(&mut self) {
        for sid in &self.engine.workbook.sheet_order {
            if let Some(sheet) = self.engine.workbook.sheets.get(sid) {
                match sheet.lattice {
                    LatticeKind::Square if self.square.sheet_id != *sid => {
                        // Only rebind the first Square encountered; respect
                        // existing binding if we've already updated.
                        self.square.sheet_id = *sid;
                    }
                    LatticeKind::HexPointy | LatticeKind::HexFlat => {
                        self.hex.sheet_id = *sid;
                    }
                    LatticeKind::Triangle => {
                        self.triangle.sheet_id = *sid;
                    }
                    LatticeKind::Voronoi => {
                        self.voronoi.sheet_id = *sid;
                        // Resync the render cache from the freshly-loaded
                        // engine config so dragged/persisted seeds show up
                        // (ADR-012 single source of truth).
                        self.voronoi_lattice = synced_voronoi_lattice(&self.engine, *sid);
                        // T-010 (C-005): the seed count may have changed —
                        // clear the visit history to avoid stale OOR refs.
                        self.voronoi_visit_history.clear();
                    }
                    _ => {}
                }
            }
        }
    }

    /// Snapshot the persistent slice of the app state for serialization
    /// alongside the workbook. See [`crate::state_io::UiSnapshot`].
    pub(crate) fn capture_state(&self) -> crate::state_io::UiSnapshot {
        use crate::state_io::{ActiveSheetTag, UiSnapshot};
        let active = match self.active {
            ActiveSheet::Square => ActiveSheetTag::Square,
            ActiveSheet::Hex => ActiveSheetTag::Hex,
            ActiveSheet::Triangle => ActiveSheetTag::Triangle,
            ActiveSheet::Voronoi => ActiveSheetTag::Voronoi,
        };
        let mut sq_formats = FormatMap::new();
        sq_formats.replace_with(self.square.formats.iter().map(|(k, v)| (*k, v.clone())));
        let mut hex_formats = FormatMap::new();
        hex_formats.replace_with(self.hex.formats.iter().map(|(k, v)| (*k, v.clone())));
        let mut tri_formats = FormatMap::new();
        tri_formats.replace_with(self.triangle.formats.iter().map(|(k, v)| (*k, v.clone())));
        let mut vor_formats = FormatMap::new();
        vor_formats.replace_with(self.voronoi.formats.iter().map(|(k, v)| (*k, v.clone())));

        let mut sq_widgets: widget::Widgets<(u32, u32)> = widget::Widgets::default();
        sq_widgets.replace_with(self.square_widgets.iter().map(|(k, v)| (*k, *v)));
        let mut hex_widgets: widget::Widgets<HexCoord> = widget::Widgets::default();
        hex_widgets.replace_with(self.hex_widgets.iter().map(|(k, v)| (*k, *v)));
        let mut tri_widgets: widget::Widgets<TriCoord> = widget::Widgets::default();
        tri_widgets.replace_with(self.triangle_widgets.iter().map(|(k, v)| (*k, *v)));
        let mut vor_widgets: widget::Widgets<VoronoiCoord> = widget::Widgets::default();
        vor_widgets.replace_with(self.voronoi_widgets.iter().map(|(k, v)| (*k, *v)));

        UiSnapshot {
            active_sheet: active,
            stage_mode: self.stage_mode,
            dark_mode: self.dark_mode,
            square_formats: sq_formats,
            hex_formats,
            triangle_formats: tri_formats,
            voronoi_formats: vor_formats,
            square_widgets: sq_widgets,
            hex_widgets,
            triangle_widgets: tri_widgets,
            voronoi_widgets: vor_widgets,
            square_notes: self
                .notes
                .iter()
                .map(|(k, v)| (*k, v.to_string()))
                .collect(),
            hex_notes: self
                .hex_notes
                .iter()
                .map(|(k, v)| (*k, v.to_string()))
                .collect(),
            triangle_notes: self
                .triangle_notes
                .iter()
                .map(|(k, v)| (*k, v.to_string()))
                .collect(),
            voronoi_notes: self
                .voronoi_notes
                .iter()
                .map(|(k, v)| (*k, v.to_string()))
                .collect(),
            conditional_rules: self.cond_rules.clone(),
            column_widths: self.metrics.col_widths_iter().collect(),
            row_heights: self.metrics.row_heights_iter().collect(),
            is_fresh_seed: false,
        }
    }

    /// Apply a previously-captured snapshot back onto the app. Engine
    /// state (cell sources / values) is not touched — that's loaded
    /// separately via [`WorkbookEngine::open_bytes`]. Ephemeral state
    /// (history, drag flags, dialogs) is reset to defaults by the caller
    /// when it constructs a fresh app before calling this.
    pub(crate) fn restore_state(&mut self, s: crate::state_io::UiSnapshot) {
        use crate::state_io::ActiveSheetTag;
        self.active = match s.active_sheet {
            ActiveSheetTag::Square => ActiveSheet::Square,
            ActiveSheetTag::Hex => ActiveSheet::Hex,
            ActiveSheetTag::Triangle => ActiveSheet::Triangle,
            ActiveSheetTag::Voronoi => ActiveSheet::Voronoi,
        };
        self.stage_mode = s.stage_mode;
        self.dark_mode = s.dark_mode;
        self.square
            .formats
            .replace_with(s.square_formats.iter().map(|(k, v)| (*k, v.clone())));
        self.hex
            .formats
            .replace_with(s.hex_formats.iter().map(|(k, v)| (*k, v.clone())));
        self.triangle
            .formats
            .replace_with(s.triangle_formats.iter().map(|(k, v)| (*k, v.clone())));
        self.voronoi
            .formats
            .replace_with(s.voronoi_formats.iter().map(|(k, v)| (*k, v.clone())));
        self.square_widgets
            .replace_with(s.square_widgets.iter().map(|(k, v)| (*k, *v)));
        self.hex_widgets
            .replace_with(s.hex_widgets.iter().map(|(k, v)| (*k, *v)));
        self.triangle_widgets
            .replace_with(s.triangle_widgets.iter().map(|(k, v)| (*k, *v)));
        self.voronoi_widgets
            .replace_with(s.voronoi_widgets.iter().map(|(k, v)| (*k, *v)));
        self.notes.replace_with(s.square_notes);
        self.hex_notes.replace_with(s.hex_notes);
        self.triangle_notes.replace_with(s.triangle_notes);
        self.voronoi_notes.replace_with(s.voronoi_notes);
        self.cond_rules = s.conditional_rules;
        self.metrics.replace_with(s.column_widths, s.row_heights);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use carbide_core::CellValue;

    /// Minimal pure-Rust uncompressed ZIP writer so `.xlsx` UI import tests
    /// need no extra dev-dependencies or binary fixtures.
    fn build_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        fn crc32(data: &[u8]) -> u32 {
            let mut crc = 0xFFFF_FFFFu32;
            for &b in data {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    let mask = (crc & 1).wrapping_neg();
                    crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
                }
            }
            !crc
        }

        let mut out = Vec::new();
        let mut central = Vec::new();
        for &(name, data) in entries {
            let name_bytes = name.as_bytes();
            let crc = crc32(data);
            let len = data.len() as u32;
            let offset = out.len() as u32;

            out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(data);

            central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&len.to_le_bytes());
            central.extend_from_slice(&len.to_le_bytes());
            central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name_bytes);
        }
        let central_offset = out.len() as u32;
        let central_len = central.len() as u32;
        out.extend_from_slice(&central);

        let count = entries.len() as u16;
        out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&central_len.to_le_bytes());
        out.extend_from_slice(&central_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn two_sheet_xlsx() -> Vec<u8> {
        let content_types = br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#;
        let root_rels = br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;
        let workbook = br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="rId1"/>
    <sheet name="Raw" sheetId="2" r:id="rId2"/>
  </sheets>
</workbook>"#;
        let wb_rels = br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>"#;
        let sheet1 = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="n"><v>12</v></c>
      <c r="B1" t="n"><v>8</v></c>
      <c r="C1"><f>A1+B1</f></c>
    </row>
  </sheetData>
</worksheet>"#;
        let sheet2 = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="n"><v>99</v></c>
    </row>
  </sheetData>
</worksheet>"#;
        build_stored_zip(&[
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", root_rels),
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", wb_rels),
            ("xl/worksheets/sheet1.xml", sheet1),
            ("xl/worksheets/sheet2.xml", sheet2),
        ])
    }

    fn seeded_four_sheet_engine() -> (WorkbookEngine, SheetId, SheetId, SheetId, SheetId) {
        let mut engine = WorkbookEngine::new();
        engine.new_workbook();
        let sq = engine.add_sheet("Budget", LatticeKind::Square);
        engine.set_cell(sq, "A1", Some("Old")).unwrap();
        let hex = engine.add_sheet("Hex Game", LatticeKind::HexPointy);
        engine
            .set_cell(hex, "H(2,2)", Some("=RANDBETWEEN(1,6)"))
            .unwrap();
        let tri = engine.add_sheet("Tri demo", LatticeKind::Triangle);
        engine.set_cell(tri, "T(2,0)", Some("=4")).unwrap();
        let vor = engine.add_sheet("Voronoi", LatticeKind::Voronoi);
        engine.set_cell(vor, "V(2)", Some("=42")).unwrap();
        (engine, sq, hex, tri, vor)
    }

    #[test]
    fn apply_imported_csv_updates_square_sheet_and_preserves_siblings() {
        let (mut engine, sq, hex, tri, vor) = seeded_four_sheet_engine();
        let hex_before = engine.get_cell(hex, "H(2,2)").unwrap().value;

        let csv = b"Item,Qty,Price\nWidget,3,12.5\nTotal,,=B2*C2\n";
        let outcome = apply_imported_bytes(&mut engine, sq, Some("sales.csv"), csv).unwrap();
        assert_eq!(outcome.sheet_name, "sales");
        assert_eq!(outcome.total_sheets, 1);
        assert_eq!(engine.workbook.sheets[&sq].name, "sales");
        assert_eq!(
            engine.get_cell(sq, "C3").unwrap().value,
            CellValue::Number(37.5)
        );
        // Sibling sheets — including Hex's volatile RANDBETWEEN — stay untouched.
        assert_eq!(engine.get_cell(hex, "H(2,2)").unwrap().value, hex_before);
        assert_eq!(
            engine.get_cell(tri, "T(2,0)").unwrap().value,
            CellValue::Number(4.0)
        );
        assert_eq!(
            engine.get_cell(vor, "V(2)").unwrap().value,
            CellValue::Number(42.0)
        );
    }

    #[test]
    fn apply_imported_multisheet_xlsx_reports_total_sheets_and_evaluates_formulas() {
        let (mut engine, sq, hex, _, _) = seeded_four_sheet_engine();
        let hex_before = engine.get_cell(hex, "H(2,2)").unwrap().value;

        let xlsx = two_sheet_xlsx();
        let outcome = apply_imported_bytes(&mut engine, sq, Some("quarterly.xlsx"), &xlsx).unwrap();
        assert_eq!(outcome.sheet_name, "Summary");
        assert_eq!(outcome.total_sheets, 2);
        assert_eq!(engine.workbook.sheets[&sq].name, "Summary");
        assert_eq!(
            engine.get_cell(sq, "C1").unwrap().value,
            CellValue::Number(20.0)
        );
        assert_eq!(engine.get_cell(hex, "H(2,2)").unwrap().value, hex_before);
    }

    #[test]
    fn apply_imported_corrupt_bytes_leaves_engine_untouched() {
        let (mut engine, sq, _, _, _) = seeded_four_sheet_engine();
        let err = apply_imported_bytes(&mut engine, sq, Some("bad.xlsx"), b"PK\x03\x04corrupt")
            .unwrap_err();
        assert!(err.contains("Excel (.xlsx) import failed"));
        assert_eq!(engine.workbook.sheets[&sq].name, "Budget");
        assert_eq!(
            engine.get_cell(sq, "A1").unwrap().value,
            CellValue::Text("Old".into())
        );
    }
}
