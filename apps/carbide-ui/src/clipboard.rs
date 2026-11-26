//! The clipboard — the one first-class copy / cut / paste object.
//!
//! Each captured cell keeps **both** its raw source and its evaluated
//! value. A paste onto the *same* kind of sheet writes the source, so
//! formulas carry; a paste onto a *different* kind writes the value as a
//! plain literal, since the source's cell references (`B2`, `H(1,0)`, …)
//! don't translate across lattices. That makes copy/paste first-class:
//! data moves between any cell type, formulas degrade gracefully to
//! their result when they cross a lattice boundary.
//!
//! **Cut is part of this same object** — not a separate per-sheet field.
//! A cut is just a captured block that also remembers its origin (grid
//! corner or explicit Voronoi cell list); the next paste clears that
//! origin atomically — whether on the same sheet or across lattices —
//! and the cut is consumed.
//!
//! No egui and no engine here, so the whole model is `cargo test`-able.

use carbide_tess::voronoi::VoronoiCoord;

/// One captured cell — its source, and its value rendered as a literal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CopiedCell {
    /// The raw source as the user typed it. `None` is a blank cell.
    pub source: Option<String>,
    /// The cell's evaluated value as a plain, re-typeable literal — what
    /// a cross-lattice paste writes in place of the (untranslatable)
    /// source. `None` when the cell evaluates to nothing.
    pub value: Option<String>,
}

/// Which form of a captured cell a paste writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    /// The captured source on the same lattice, the value across one —
    /// the ordinary paste, so a formula carries wherever it still can.
    Normal,
    /// The evaluated value always, even on the same lattice — a paste
    /// that strips formulas down to their result.
    ValuesOnly,
}

/// Which lattice a captured block came from. A paste compares this
/// against the destination lattice to choose source vs. value — same
/// kind keeps the formula, different kind degrades to the literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceLattice {
    #[default]
    Square,
    Hex,
    Triangle,
    Voronoi,
}

/// Origin cells armed for clearing when the next paste lands.
/// Grid-indexed lattices (Square, Hex, Triangle) record the leading
/// `(col, row)` / `(q, r)` corner and combine it with `(width, height)`.
/// Voronoi records the exact list of cut cells so non-contiguous marquee
/// selections clear only the cells that were actually cut.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CutOrigin {
    Grid(i32, i32),
    Voronoi(Vec<VoronoiCoord>),
}

/// A rectangular block of captured cells, stored row-major — and, when
/// the block was cut rather than copied, the origin to clear on paste.
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    width: u32,
    height: u32,
    cells: Vec<CopiedCell>,
    /// Which lattice the block was captured from. The paste compares
    /// this against the destination to decide source-vs-value.
    source: SourceLattice,
    /// `Some(...)` when this block was **cut** — `None` for a plain copy.
    cut: Option<CutOrigin>,
}

impl Clipboard {
    /// Capture a `width × height` block, row-major, as a **copy**.
    /// `source` records which lattice the block came from.
    pub fn capture(width: u32, height: u32, cells: Vec<CopiedCell>, source: SourceLattice) -> Self {
        debug_assert_eq!(cells.len(), (width * height) as usize);
        Self {
            width,
            height,
            cells,
            source,
            cut: None,
        }
    }

    /// Turn a freshly-captured block into a **cut**: record the leading
    /// corner of the origin region so the next paste can clear it.
    pub fn mark_as_cut(&mut self, origin: (i32, i32)) {
        self.cut = Some(CutOrigin::Grid(origin.0, origin.1));
    }

    /// Turn a freshly-captured Voronoi block into a **cut**, recording the
    /// exact cells (including non-contiguous marquee selections) to clear.
    pub fn mark_as_voronoi_cut(&mut self, cells: Vec<VoronoiCoord>) {
        self.cut = Some(CutOrigin::Voronoi(cells));
    }

    /// Whether the clipboard holds nothing.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// `(width, height)` of the captured block.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Which lattice the block was captured from.
    pub fn source(&self) -> SourceLattice {
        self.source
    }

    /// Convenience: whether the block was captured from the hex sheet.
    pub fn from_hex(&self) -> bool {
        matches!(self.source, SourceLattice::Hex)
    }

    /// Convenience: whether the block was captured from the triangle sheet.
    pub fn from_triangle(&self) -> bool {
        matches!(self.source, SourceLattice::Triangle)
    }

    /// Convenience: whether the block was captured from the Voronoi sheet.
    pub fn from_voronoi(&self) -> bool {
        matches!(self.source, SourceLattice::Voronoi)
    }

    /// Whether any cut (grid or Voronoi) is currently armed.
    pub fn has_cut(&self) -> bool {
        self.cut.is_some()
    }

    /// The leading corner of a grid cut origin region — `Some` only when
    /// this block was cut from a grid-indexed lattice (Square/Hex/Triangle).
    pub fn cut_origin(&self) -> Option<(i32, i32)> {
        match &self.cut {
            Some(CutOrigin::Grid(a, b)) => Some((*a, *b)),
            _ => None,
        }
    }

    /// The exact cells armed for clearing when this block was cut from Voronoi.
    pub fn cut_voronoi_cells(&self) -> Option<&[VoronoiCoord]> {
        match &self.cut {
            Some(CutOrigin::Voronoi(cells)) => Some(cells.as_slice()),
            _ => None,
        }
    }

    /// Drop the cut origin — a paste (or Escape) has consumed the cut, so
    /// further pastes of this block behave as a plain copy.
    pub fn consume_cut(&mut self) {
        self.cut = None;
    }

    /// Every captured cell as `(rel_col, rel_row, &cell)`, row-major.
    pub fn entries(&self) -> impl Iterator<Item = (u32, u32, &CopiedCell)> {
        let width = self.width;
        self.cells.iter().enumerate().map(move |(i, cell)| {
            let i = i as u32;
            (i % width, i / width, cell)
        })
    }

    /// What to write when pasting `cell` onto `target`. Same lattice as
    /// the capture → the source, so formulas carry; different lattice →
    /// the value, since the source can't translate across lattices.
    pub fn source_for(&self, cell: &CopiedCell, target: SourceLattice) -> Option<String> {
        if self.source == target {
            cell.source.clone()
        } else {
            cell.value.clone()
        }
    }

    /// The text a paste of `cell` writes under `mode`. `Normal` defers to
    /// [`source_for`](Self::source_for); `ValuesOnly` writes the evaluated
    /// value even on the same lattice, so the paste drops the formula.
    pub fn paste_text(
        &self,
        cell: &CopiedCell,
        target: SourceLattice,
        mode: PasteMode,
    ) -> Option<String> {
        match mode {
            PasteMode::Normal => self.source_for(cell, target),
            PasteMode::ValuesOnly => cell.value.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A formula cell: a source plus the value it evaluates to.
    fn formula(source: &str, value: &str) -> CopiedCell {
        CopiedCell {
            source: Some(source.to_string()),
            value: Some(value.to_string()),
        }
    }

    #[test]
    fn a_default_clipboard_is_empty() {
        let c = Clipboard::default();
        assert!(c.is_empty());
        assert_eq!(c.dimensions(), (0, 0));
        assert_eq!(c.entries().count(), 0);
        assert_eq!(c.cut_origin(), None);
    }

    #[test]
    fn capture_records_dimensions_and_entries() {
        let cells = vec![
            formula("=1", "1"),
            CopiedCell::default(),
            formula("=2", "2"),
            formula("=3", "3"),
        ];
        let c = Clipboard::capture(2, 2, cells, SourceLattice::Square);
        assert!(!c.is_empty());
        assert_eq!(c.dimensions(), (2, 2));
        let got: Vec<_> = c.entries().collect();
        assert_eq!(got.len(), 4);
        assert_eq!(got[1], (1, 0, &CopiedCell::default()));
        assert_eq!(got[3].2.source.as_deref(), Some("=3"));
    }

    #[test]
    fn a_plain_capture_is_a_copy_not_a_cut() {
        let c = Clipboard::capture(1, 1, vec![formula("=1", "1")], SourceLattice::Square);
        assert_eq!(c.cut_origin(), None);
    }

    #[test]
    fn mark_as_cut_then_consume() {
        let mut c = Clipboard::capture(1, 1, vec![formula("=1", "1")], SourceLattice::Hex);
        c.mark_as_cut((2, -3));
        assert_eq!(c.cut_origin(), Some((2, -3)));
        assert!(c.from_hex());
        // A paste consumes the cut — afterwards it is a plain copy.
        c.consume_cut();
        assert_eq!(c.cut_origin(), None);
    }

    #[test]
    fn source_records_the_capture_lattice() {
        let c = Clipboard::capture(1, 1, vec![formula("=1", "1")], SourceLattice::Triangle);
        assert!(c.from_triangle());
        assert!(!c.from_hex());
        assert_eq!(c.source(), SourceLattice::Triangle);
    }

    #[test]
    fn same_lattice_paste_keeps_the_source() {
        let c = Clipboard::capture(
            1,
            1,
            vec![formula("=SUM(B2:B4)", "60")],
            SourceLattice::Square,
        );
        let (.., cell) = c.entries().next().unwrap();
        assert_eq!(
            c.source_for(cell, SourceLattice::Square),
            Some("=SUM(B2:B4)".to_string()),
        );
    }

    #[test]
    fn cross_lattice_paste_degrades_to_the_value() {
        // Copied from the square sheet, pasted onto the hex sheet — the
        // formula's references can't translate, so the value is written.
        let c = Clipboard::capture(
            1,
            1,
            vec![formula("=SUM(B2:B4)", "60")],
            SourceLattice::Square,
        );
        let (.., cell) = c.entries().next().unwrap();
        assert_eq!(
            c.source_for(cell, SourceLattice::Hex),
            Some("60".to_string()),
        );

        // And the other direction: hex capture pasted onto the square sheet.
        let h = Clipboard::capture(1, 1, vec![formula("=H(1,0)", "12")], SourceLattice::Hex);
        let (.., hcell) = h.entries().next().unwrap();
        assert_eq!(
            h.source_for(hcell, SourceLattice::Square),
            Some("12".to_string()),
        );
        assert_eq!(
            h.source_for(hcell, SourceLattice::Hex),
            Some("=H(1,0)".to_string()),
        );

        // Triangle is symmetric — same lattice keeps the source, others
        // degrade to the value.
        let t = Clipboard::capture(
            1,
            1,
            vec![formula("=T(0,0)+1", "5")],
            SourceLattice::Triangle,
        );
        let (.., tcell) = t.entries().next().unwrap();
        assert_eq!(
            t.source_for(tcell, SourceLattice::Triangle),
            Some("=T(0,0)+1".to_string()),
        );
        assert_eq!(
            t.source_for(tcell, SourceLattice::Square),
            Some("5".to_string()),
        );
    }

    #[test]
    fn values_only_paste_drops_the_formula_on_the_same_lattice() {
        let c = Clipboard::capture(
            1,
            1,
            vec![formula("=SUM(B2:B4)", "60")],
            SourceLattice::Square,
        );
        let (.., cell) = c.entries().next().unwrap();
        // Normal: a same-lattice paste keeps the source.
        assert_eq!(
            c.paste_text(cell, SourceLattice::Square, PasteMode::Normal),
            Some("=SUM(B2:B4)".to_string()),
        );
        // ValuesOnly: the evaluated value, even on the same lattice.
        assert_eq!(
            c.paste_text(cell, SourceLattice::Square, PasteMode::ValuesOnly),
            Some("60".to_string()),
        );
        // Across a lattice the two modes agree — a source can't carry.
        assert_eq!(
            c.paste_text(cell, SourceLattice::Hex, PasteMode::Normal),
            c.paste_text(cell, SourceLattice::Hex, PasteMode::ValuesOnly),
        );
    }

    #[test]
    fn voronoi_source_keeps_formula_on_voronoi_and_degrades_cross_lattice() {
        let v = Clipboard::capture(
            1,
            1,
            vec![formula("=SUM(V(0):V(2))", "99")],
            SourceLattice::Voronoi,
        );
        assert!(v.from_voronoi());
        assert!(!v.from_hex());
        assert!(!v.from_triangle());
        assert_eq!(v.source(), SourceLattice::Voronoi);
        let (.., cell) = v.entries().next().unwrap();
        assert_eq!(
            v.paste_text(cell, SourceLattice::Voronoi, PasteMode::Normal),
            Some("=SUM(V(0):V(2))".to_string()),
        );
        assert_eq!(
            v.paste_text(cell, SourceLattice::Voronoi, PasteMode::ValuesOnly),
            Some("99".to_string()),
        );
        assert_eq!(
            v.paste_text(cell, SourceLattice::Square, PasteMode::Normal),
            Some("99".to_string()),
        );
    }

    #[test]
    fn voronoi_non_contiguous_cut_tracks_exact_cells_and_consumes() {
        let mut v = Clipboard::capture(
            1,
            2,
            vec![formula("=V(0)", "1"), formula("=V(3)", "4")],
            SourceLattice::Voronoi,
        );
        assert!(!v.has_cut());
        assert_eq!(v.cut_voronoi_cells(), None);
        v.mark_as_voronoi_cut(vec![VoronoiCoord(0), VoronoiCoord(3)]);
        assert!(v.has_cut());
        assert_eq!(v.cut_origin(), None);
        assert_eq!(
            v.cut_voronoi_cells(),
            Some(&[VoronoiCoord(0), VoronoiCoord(3)][..])
        );
        v.consume_cut();
        assert!(!v.has_cut());
        assert_eq!(v.cut_voronoi_cells(), None);
    }
}
