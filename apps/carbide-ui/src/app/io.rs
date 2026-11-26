//! Persistence I/O — `.crbd`/`.tscl` save/open dialogs, wasm localStorage autosave,
//! sheet ID rebinding, and `UiSnapshot` capture/restore.

use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use carbide_tess::voronoi::VoronoiCoord;
use carbide_tess::LatticeKind;

use crate::format::FormatMap;
use crate::widget;

use super::types::ActiveSheet;
use super::voronoi_helpers::synced_voronoi_lattice;
use super::CarbideApp;

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
                    .add_filter("Carbide", &["tscl"])
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
                    .add_filter("Carbide", &["tscl"])
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
                // Don't immediately autosave the just-loaded state.
                self.dirty = false;
                self.last_autosave = self.frame_time;
                self.suppress_autosave_until = self.frame_time + 2.0;
            }
            Err(e) => eprintln!("open: engine.open_bytes failed: {e:?}"),
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
        self.cond_rules = s.conditional_rules;
        self.metrics.replace_with(s.column_widths, s.row_heights);
    }
}
