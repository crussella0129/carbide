//! Pure Voronoi interaction helpers — seed drag, proximity fade, widget pop-out,
//! Enter-advance history walk, and screen-rect marquee selection.

use carbide_core::SheetId;
use carbide_formula::WorkbookEngine;
use carbide_tess::voronoi::{VoronoiCoord, VoronoiLattice};
use carbide_tess::{Lattice, Point2};
use eframe::egui;

use crate::clipboard::{Clipboard, PasteMode, SourceLattice};

/// The `V(N)` address string for a Voronoi coord — matches the
/// canonical form `carbide-tess`'s voronoi lattice emits.
pub(super) fn voronoi_address(c: VoronoiCoord) -> String {
    format!("V({})", c.0)
}

/// Inset (lattice units) kept between a dragged seed and the bounding box,
/// so a seed can't be dragged onto the boundary where its cell degenerates.
pub(super) const VORONOI_DRAG_INSET: f32 = 2.0;

/// Apply a seed-handle drag: return a copy of `seeds` with element `idx`
/// translated by `delta` and clamped to `bounds` (`[min_x, min_y, max_x,
/// max_y]`) minus [`VORONOI_DRAG_INSET`]. An out-of-range `idx` returns the
/// seeds unchanged.
///
/// Pure (no egui) so the clamp is unit-testable. It does NOT check seed
/// coincidence — that's the engine's `set_voronoi_seeds` job, which rejects
/// (snap-back) on a degenerate result. Assumes 1 lattice unit == 1 screen
/// pixel with no zoom (the Voronoi renderer applies no scale); a future
/// zoom feature must scale `delta` before calling this.
pub(super) fn apply_seed_drag(
    seeds: &[[f32; 2]],
    idx: usize,
    delta: [f32; 2],
    bounds: [f32; 4],
) -> Vec<[f32; 2]> {
    let mut out = seeds.to_vec();
    if let Some(s) = out.get_mut(idx) {
        let min_x = bounds[0] + VORONOI_DRAG_INSET;
        let min_y = bounds[1] + VORONOI_DRAG_INSET;
        let max_x = bounds[2] - VORONOI_DRAG_INSET;
        let max_y = bounds[3] - VORONOI_DRAG_INSET;
        s[0] = (s[0] + delta[0]).clamp(min_x, max_x);
        s[1] = (s[1] + delta[1]).clamp(min_y, max_y);
    }
    out
}

/// The Voronoi render cache for `sid`, read from the engine (the single
/// source of truth for seed geometry, ADR-012). Falls back to the default
/// lattice if the sheet has no Voronoi config. Pure (no egui) so the
/// load/post-drag resync is unit-testable.
pub(super) fn synced_voronoi_lattice(engine: &WorkbookEngine, sid: SheetId) -> VoronoiLattice {
    engine.voronoi_lattice(sid).unwrap_or_default()
}

/// Every Voronoi cell whose centroid (in screen space, i.e. `origin +
/// lattice_centroid`) falls inside `screen_rect`. The unified marquee rule
/// for a tessellation without rect-indexed coords (ADR-013). A degenerate
/// rect (zero width or height) returns an empty `Vec`.
/// Voronoi seed-handle proximity fade thresholds (ADR-014, lattice units).
/// Within `INNER` of the cursor the handle is fully opaque; past `OUTER`
/// it's invisible; between the two it fades linearly.
pub(super) const SEED_FADE_INNER: f32 = 40.0;
pub(super) const SEED_FADE_OUTER: f32 = 80.0;
/// Maximum alpha for the seed-handle fill (below 255 so the dot reads as
/// a marker rather than a fully-opaque blob over cell content).
pub(super) const SEED_FADE_MAX_ALPHA: u8 = 225;

/// Distance-to-alpha mapping for the seed-handle proximity fade (ADR-014).
/// Piecewise: `[<=INNER] = MAX`, `[>=OUTER] = 0`, linear in between. Pure
/// (no egui types) so the per-frame per-seed decision is unit-testable
/// without a render context.
pub(super) fn fade_alpha(distance: f32) -> u8 {
    if distance <= SEED_FADE_INNER {
        SEED_FADE_MAX_ALPHA
    } else if distance >= SEED_FADE_OUTER {
        0
    } else {
        ((SEED_FADE_OUTER - distance) / (SEED_FADE_OUTER - SEED_FADE_INNER)
            * SEED_FADE_MAX_ALPHA as f32) as u8
    }
}

/// Per-frame per-seed alpha for the Voronoi handle proximity fade
/// (ADR-014 / C-008 partial). Priority order: **dragged > fade > no-hover.**
///
/// - Any seed at `dragging_index` always gets full opacity (the user is
///   actively interacting with it, mustn't disappear if the pointer leaves
///   the panel mid-drag).
/// - Otherwise, with a cursor present, each seed's alpha is
///   `fade_alpha(distance(seed, cursor))`.
/// - With no cursor and no drag, every seed is invisible (alpha `0`).
///
/// Pure (no egui types beyond `Pos2`) so the per-frame decision is
/// unit-testable without a render context.
pub(super) fn seed_handle_alphas(
    cursor: Option<egui::Pos2>,
    seeds_screen: &[egui::Pos2],
    dragging_index: Option<usize>,
) -> Vec<u8> {
    seeds_screen
        .iter()
        .enumerate()
        .map(|(i, seed)| {
            if Some(i) == dragging_index {
                SEED_FADE_MAX_ALPHA
            } else if let Some(c) = cursor {
                fade_alpha((*seed - c).length())
            } else {
                0
            }
        })
        .collect()
}

/// Pop-out offset distance (lattice units) for a widget whose centroid
/// would otherwise sit under a visible seed handle. Larger than the
/// handle's 5px hit radius plus a comfort margin (ADR-014).
pub(super) const WIDGET_POP_OUT_OFFSET: f32 = 30.0;

/// When a Voronoi widget cell's seed handle is visible (cursor near), the
/// widget moves to a side of the centroid so its click target isn't
/// occluded by the handle (ADR-014). Tries `[+x, -x, +y, -y]` offsets at
/// `offset` (lattice units), returns the first center where
/// `cell_at(candidate) == Some(seed_coord)` (i.e. the candidate still
/// falls in this cell's polygon). Falls back to `centroid` when no side
/// has clearance.
///
/// **Coord space:** all positions are lattice-space. The caller maps the
/// returned `Vec2` to screen via `egui::pos2(origin.x + result.x, origin.y
/// + result.y)`. Precondition: 1 lattice unit == 1 screen pixel with no
/// zoom (ADR-013 C-006); a future zoom feature must scale `offset` first.
pub(super) fn pop_out_widget_center<F: Fn(Point2) -> Option<VoronoiCoord>>(
    centroid: Point2,
    offset: f32,
    seed_coord: VoronoiCoord,
    cell_at: F,
) -> Point2 {
    let candidates = [
        Point2::new(centroid.x + offset, centroid.y), // right
        Point2::new(centroid.x - offset, centroid.y), // left
        Point2::new(centroid.x, centroid.y + offset), // down
        Point2::new(centroid.x, centroid.y - offset), // up
    ];
    candidates
        .into_iter()
        .find(|&p| cell_at(p) == Some(seed_coord))
        .unwrap_or(centroid)
}

/// Pick the next Voronoi cell for an Enter-advance traversal (ADR-014 F3).
/// Filters `candidates` to those that are NOT `current` (defensive — a
/// well-formed `Lattice::neighbors` never returns self, but we exclude it
/// anyway) and NOT in `history`. From the survivors, picks the one whose
/// centroid is closest to `current_centroid` (Euclidean distance); ties
/// break by lowest `VoronoiCoord` index for determinism. Returns `None`
/// when no candidate qualifies — Enter stops at that point.
///
/// Pure (no engine, no egui) so the history-traversal decision is
/// unit-testable. The caller assembles `(coord, centroid)` pairs from
/// `Lattice::neighbors(current)` + `Lattice::centroid(neighbor)`.
pub(super) fn pick_next_voronoi(
    current: VoronoiCoord,
    current_centroid: Point2,
    candidates: &[(VoronoiCoord, Point2)],
    history: &[VoronoiCoord],
) -> Option<VoronoiCoord> {
    candidates
        .iter()
        .filter(|(c, _)| *c != current && !history.contains(c))
        .min_by(|(a_c, a_p), (b_c, b_p)| {
            let da = (*a_p - current_centroid).length_squared();
            let db = (*b_p - current_centroid).length_squared();
            // Tie-break by lowest index when distances are equal.
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a_c.0.cmp(&b_c.0))
        })
        .map(|(c, _)| *c)
}

/// Apply one Voronoi Enter-advance step (ADR-014 F3). Pure-over-engine
/// so the history mutation + bounded-queue cap is testable with just a
/// `WorkbookEngine` (no full `CarbideApp`). Returns the next cursor
/// `VoronoiCoord` to collapse to, or `None` if Enter should stop.
/// Mutates `history` in place: on a successful pick, pushes `current`
/// onto the back and pops the front while length > `N-1`.
///
/// Defensive: returns `None` (and leaves history untouched) if `current`
/// is out of range for the current lattice — e.g. after a workbook reload
/// to a smaller seed set before `rebind_sheet_ids` clears the history.
pub(super) fn voronoi_advance_step(
    engine: &WorkbookEngine,
    sheet: SheetId,
    current: VoronoiCoord,
    history: &mut std::collections::VecDeque<VoronoiCoord>,
) -> Option<VoronoiCoord> {
    let lattice = engine.voronoi_lattice(sheet)?;
    if (current.0 as usize) >= lattice.len() {
        return None;
    }
    let current_centroid = lattice.centroid(current);
    let candidates: Vec<(VoronoiCoord, Point2)> = lattice
        .neighbors(current)
        .into_iter()
        .map(|(_dir, c)| (c, lattice.centroid(c)))
        .collect();
    let history_vec: Vec<VoronoiCoord> = history.iter().copied().collect();
    let pick = pick_next_voronoi(current, current_centroid, &candidates, &history_vec)?;
    let cap = lattice.len().saturating_sub(1);
    history.push_back(current);
    while history.len() > cap {
        history.pop_front();
    }
    Some(pick)
}

pub(super) fn cells_in_screen_rect(
    lattice: &VoronoiLattice,
    screen_rect: egui::Rect,
    origin: egui::Pos2,
) -> Vec<VoronoiCoord> {
    if screen_rect.width() <= 0.0 || screen_rect.height() <= 0.0 {
        return Vec::new();
    }
    (0..lattice.len() as u32)
        .map(VoronoiCoord)
        .filter(|&coord| {
            let c = lattice.centroid(coord);
            let p = egui::pos2(origin.x + c.x, origin.y + c.y);
            screen_rect.contains(p)
        })
        .collect()
}

/// Build the destination writes and written coordinates when pasting
/// `clipboard` onto a Voronoi sheet starting at `cursor` (`V(cursor.0 + i)`),
/// truncating cleanly at `lattice_len`. Same-lattice `PasteMode::Normal`
/// preserves raw formulas; cross-lattice or `PasteMode::ValuesOnly` writes
/// evaluated literals via `Clipboard::paste_text`.
pub(super) fn voronoi_paste_targets(
    clipboard: &Clipboard,
    cursor: VoronoiCoord,
    lattice_len: u32,
    mode: PasteMode,
) -> (Vec<(String, Option<String>)>, Vec<VoronoiCoord>) {
    let mut targets = Vec::new();
    let mut coords = Vec::new();
    for (i, (_, _, cell)) in clipboard.entries().enumerate() {
        let Some(idx) = cursor.0.checked_add(i as u32) else {
            break;
        };
        if idx >= lattice_len {
            break;
        }
        let coord = VoronoiCoord(idx);
        let source = clipboard.paste_text(cell, SourceLattice::Voronoi, mode);
        targets.push((voronoi_address(coord), source));
        coords.push(coord);
    }
    (targets, coords)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::CopiedCell;

    fn formula(source: &str, value: &str) -> CopiedCell {
        CopiedCell {
            source: Some(source.to_string()),
            value: Some(value.to_string()),
        }
    }

    #[test]
    fn voronoi_paste_targets_same_lattice_preserves_formulas_and_selection() {
        let clip = Clipboard::capture(
            1,
            2,
            vec![formula("=V(0)+1", "11"), formula("=V(1)*2", "20")],
            SourceLattice::Voronoi,
        );
        let (targets, coords) = voronoi_paste_targets(&clip, VoronoiCoord(3), 8, PasteMode::Normal);
        assert_eq!(
            targets,
            vec![
                ("V(3)".to_string(), Some("=V(0)+1".to_string())),
                ("V(4)".to_string(), Some("=V(1)*2".to_string())),
            ]
        );
        assert_eq!(coords, vec![VoronoiCoord(3), VoronoiCoord(4)]);
    }

    #[test]
    fn voronoi_paste_targets_cross_lattice_and_values_only_degrade_to_literals() {
        let from_square = Clipboard::capture(
            2,
            1,
            vec![formula("=SUM(A1:A3)", "42"), formula("=B1", "hi")],
            SourceLattice::Square,
        );
        let (targets, coords) =
            voronoi_paste_targets(&from_square, VoronoiCoord(1), 8, PasteMode::Normal);
        assert_eq!(
            targets,
            vec![
                ("V(1)".to_string(), Some("42".to_string())),
                ("V(2)".to_string(), Some("hi".to_string())),
            ]
        );
        assert_eq!(coords, vec![VoronoiCoord(1), VoronoiCoord(2)]);

        let from_voronoi = Clipboard::capture(
            1,
            1,
            vec![formula("=V(0)*10", "50")],
            SourceLattice::Voronoi,
        );
        let (val_targets, _) =
            voronoi_paste_targets(&from_voronoi, VoronoiCoord(0), 8, PasteMode::ValuesOnly);
        assert_eq!(
            val_targets,
            vec![("V(0)".to_string(), Some("50".to_string()))]
        );
    }

    #[test]
    fn voronoi_paste_targets_truncates_at_lattice_len() {
        let clip = Clipboard::capture(
            1,
            3,
            vec![formula("1", "1"), formula("2", "2"), formula("3", "3")],
            SourceLattice::Voronoi,
        );
        let (targets, coords) = voronoi_paste_targets(&clip, VoronoiCoord(6), 8, PasteMode::Normal);
        assert_eq!(
            targets,
            vec![
                ("V(6)".to_string(), Some("1".to_string())),
                ("V(7)".to_string(), Some("2".to_string())),
            ]
        );
        assert_eq!(coords, vec![VoronoiCoord(6), VoronoiCoord(7)]);
    }
}
