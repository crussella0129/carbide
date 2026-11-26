use carbide_core::{CellError, CellValue};
use carbide_formula::WorkbookEngine;
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use carbide_tess::voronoi::{VoronoiCoord, VoronoiLattice};
use carbide_tess::{LatticeKind, Point2};
use eframe::egui;

use crate::format::{CellFormat, HAlign};
use crate::keymap::Dir;

use super::helpers::*;
use super::types::*;
use super::voronoi_helpers::*;
use super::{COLS, ROWS, TRIANGLE_RADIUS};

#[test]
fn apply_seed_drag_translates_indexed_seed() {
    let seeds = vec![[0.0, 0.0], [10.0, 10.0], [-5.0, 5.0]];
    let out = apply_seed_drag(&seeds, 1, [3.0, -4.0], [-100.0, -100.0, 100.0, 100.0]);
    assert_eq!(out[1], [13.0, 6.0]);
    // Other seeds untouched.
    assert_eq!(out[0], seeds[0]);
    assert_eq!(out[2], seeds[2]);
}

#[test]
fn apply_seed_drag_clamps_to_bounds() {
    let seeds = vec![[0.0, 0.0]];
    // A huge delta is pinned to the inset boundary, never outside.
    let out = apply_seed_drag(&seeds, 0, [9999.0, -9999.0], [-50.0, -50.0, 50.0, 50.0]);
    assert_eq!(out[0][0], 50.0 - VORONOI_DRAG_INSET);
    assert_eq!(out[0][1], -50.0 + VORONOI_DRAG_INSET);
}

#[test]
fn apply_seed_drag_out_of_range_idx_noop() {
    let seeds = vec![[1.0, 2.0], [3.0, 4.0]];
    let out = apply_seed_drag(&seeds, 9, [5.0, 5.0], [-100.0, -100.0, 100.0, 100.0]);
    assert_eq!(out, seeds);
}

// --- T-005: cells_in_screen_rect (centroid-in-rect marquee) ---

fn marquee_lattice() -> VoronoiLattice {
    use carbide_tess::Rect;
    let seeds = vec![
        Point2::new(0.0, 0.0),
        Point2::new(50.0, 0.0),
        Point2::new(0.0, 50.0),
        Point2::new(50.0, 50.0),
    ];
    let bounds = Rect {
        min: Point2::new(-100.0, -100.0),
        max: Point2::new(100.0, 100.0),
    };
    VoronoiLattice::new(seeds, bounds).unwrap()
}

#[test]
fn cells_in_screen_rect_includes_centroids_inside_rect() {
    // origin=(100,100) → screen centroids at (100,100), (150,100),
    // (100,150), (150,150). A thin horizontal band y∈[95..105] catches
    // the two cells with y≈100: V(0) and V(1).
    let lat = marquee_lattice();
    let origin = egui::pos2(100.0, 100.0);
    let rect = egui::Rect::from_min_max(egui::pos2(95.0, 95.0), egui::pos2(155.0, 105.0));
    let hit = cells_in_screen_rect(&lat, rect, origin);
    assert_eq!(hit, vec![VoronoiCoord(0), VoronoiCoord(1)]);
}

#[test]
fn cells_in_screen_rect_excludes_centroids_outside_rect() {
    let lat = marquee_lattice();
    let origin = egui::pos2(100.0, 100.0);
    let rect = egui::Rect::from_min_max(egui::pos2(200.0, 200.0), egui::pos2(300.0, 300.0));
    assert!(cells_in_screen_rect(&lat, rect, origin).is_empty());
}

#[test]
fn cells_in_screen_rect_degenerate_rect_returns_empty() {
    let lat = marquee_lattice();
    let origin = egui::pos2(100.0, 100.0);
    let rect = egui::Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(100.0, 100.0));
    assert!(cells_in_screen_rect(&lat, rect, origin).is_empty());
}

#[test]
fn cells_in_screen_rect_returns_non_contiguous_indices_in_order() {
    // C-007: pick a rect that hits ONLY V(0) (centroid 100,100) and
    // V(2) (centroid 100,150) — V(1) at 150,100 is excluded. Proves the
    // iteration order is `0..len` (not "first N matched" or sorted).
    let lat = marquee_lattice();
    let origin = egui::pos2(100.0, 100.0);
    // A narrow vertical band at x≈100 catches V(0) and V(2) but not
    // V(1) at x=150 nor V(3) at x=150.
    let rect = egui::Rect::from_min_max(egui::pos2(95.0, 95.0), egui::pos2(105.0, 155.0));
    let hits = cells_in_screen_rect(&lat, rect, origin);
    assert_eq!(hits, vec![VoronoiCoord(0), VoronoiCoord(2)]);
}

// --- T-001: fade_alpha (proximity-fade thresholds) ---

#[test]
fn fade_alpha_zero_distance_is_full() {
    assert_eq!(fade_alpha(0.0), SEED_FADE_MAX_ALPHA);
}

#[test]
fn fade_alpha_inner_threshold_is_full() {
    assert_eq!(fade_alpha(SEED_FADE_INNER), SEED_FADE_MAX_ALPHA);
}

#[test]
fn fade_alpha_outer_threshold_is_zero() {
    assert_eq!(fade_alpha(SEED_FADE_OUTER), 0);
}

#[test]
fn fade_alpha_midpoint_is_half() {
    let mid = (SEED_FADE_INNER + SEED_FADE_OUTER) / 2.0; // 60.0
    let got = fade_alpha(mid) as i16;
    let target = (SEED_FADE_MAX_ALPHA as i16) / 2; // 112
    assert!(
        (got - target).abs() <= 2,
        "got {got}, expected within ±2 of {target}"
    );
}

#[test]
fn fade_alpha_past_outer_is_zero() {
    assert_eq!(fade_alpha(150.0), 0);
    assert_eq!(fade_alpha(10_000.0), 0);
}

#[test]
fn fade_alpha_negative_distance_clamps_to_full() {
    // Defensive: a hypothetical caller passing a negative distance
    // (signed delta etc.) still gets full opacity, not a wraparound.
    assert_eq!(fade_alpha(-5.0), SEED_FADE_MAX_ALPHA);
    assert_eq!(fade_alpha(-1000.0), SEED_FADE_MAX_ALPHA);
}

// --- T-002: seed_handle_alphas (priority order: dragged > fade > no-hover) ---

#[test]
fn seed_handle_alphas_no_hover_returns_all_zero() {
    let seeds = vec![
        egui::pos2(0.0, 0.0),
        egui::pos2(50.0, 0.0),
        egui::pos2(100.0, 0.0),
    ];
    let alphas = seed_handle_alphas(None, &seeds, None);
    assert_eq!(alphas, vec![0u8, 0, 0]);
}

#[test]
fn seed_handle_alphas_dragging_overrides_fade() {
    // No hover, but seed 2 is being dragged → that seed is fully opaque
    // regardless. Others honor the no-hover rule (= 0). C-007: explicit
    // 0-assertions for the non-dragged seeds so a regression that gave
    // dragged-state opacity to ALL seeds would be caught.
    let seeds = vec![
        egui::pos2(0.0, 0.0),
        egui::pos2(50.0, 0.0),
        egui::pos2(500.0, 500.0),
    ];
    let alphas = seed_handle_alphas(None, &seeds, Some(2));
    assert_eq!(alphas[0], 0, "seed 0 not dragged + no hover → 0");
    assert_eq!(alphas[1], 0, "seed 1 not dragged + no hover → 0");
    assert_eq!(
        alphas[2], SEED_FADE_MAX_ALPHA,
        "seed 2 is dragged → full opacity"
    );
}

#[test]
fn seed_handle_alphas_inner_threshold_full() {
    // Cursor sitting on seed 1 → seed 1 fully opaque, seed 0 distant
    // but within outer (50px → in the ramp), seed 2 past outer.
    let seeds = vec![
        egui::pos2(0.0, 0.0),
        egui::pos2(50.0, 0.0),
        egui::pos2(200.0, 0.0),
    ];
    let alphas = seed_handle_alphas(Some(egui::pos2(50.0, 0.0)), &seeds, None);
    assert_eq!(alphas[1], SEED_FADE_MAX_ALPHA, "seed at cursor is full");
    assert!(
        alphas[0] > 0 && alphas[0] < SEED_FADE_MAX_ALPHA,
        "seed 0 in ramp"
    );
    assert_eq!(alphas[2], 0, "seed 2 past outer");
}

// --- T-003: pop_out_widget_center (R → L → D → U → centroid fallback) ---

#[test]
fn pop_out_widget_center_picks_right_first() {
    let centroid = Point2::new(100.0, 100.0);
    let seed = VoronoiCoord(7);
    // `cell_at` accepts only the +x candidate.
    let cell_at = |p: Point2| {
        if (p.x - 130.0).abs() < 0.1 && (p.y - 100.0).abs() < 0.1 {
            Some(seed)
        } else {
            None
        }
    };
    let result = pop_out_widget_center(centroid, 30.0, seed, cell_at);
    assert_eq!(result, Point2::new(130.0, 100.0));
}

#[test]
fn pop_out_widget_center_falls_back_to_left() {
    let centroid = Point2::new(100.0, 100.0);
    let seed = VoronoiCoord(7);
    let cell_at = |p: Point2| {
        // Accept only -x (left).
        if (p.x - 70.0).abs() < 0.1 && (p.y - 100.0).abs() < 0.1 {
            Some(seed)
        } else {
            None
        }
    };
    let result = pop_out_widget_center(centroid, 30.0, seed, cell_at);
    assert_eq!(result, Point2::new(70.0, 100.0));
}

#[test]
fn pop_out_widget_center_falls_back_to_down() {
    let centroid = Point2::new(100.0, 100.0);
    let seed = VoronoiCoord(7);
    let cell_at = |p: Point2| {
        // Accept only +y (down).
        if (p.x - 100.0).abs() < 0.1 && (p.y - 130.0).abs() < 0.1 {
            Some(seed)
        } else {
            None
        }
    };
    let result = pop_out_widget_center(centroid, 30.0, seed, cell_at);
    assert_eq!(result, Point2::new(100.0, 130.0));
}

#[test]
fn pop_out_widget_center_falls_back_to_up() {
    let centroid = Point2::new(100.0, 100.0);
    let seed = VoronoiCoord(7);
    let cell_at = |p: Point2| {
        // Accept only -y (up).
        if (p.x - 100.0).abs() < 0.1 && (p.y - 70.0).abs() < 0.1 {
            Some(seed)
        } else {
            None
        }
    };
    let result = pop_out_widget_center(centroid, 30.0, seed, cell_at);
    assert_eq!(result, Point2::new(100.0, 70.0));
}

// --- T-009: pick_next_voronoi (history + closest-centroid + tie-break) ---

#[test]
fn pick_next_voronoi_all_in_history_returns_none() {
    let current = VoronoiCoord(0);
    let current_centroid = Point2::new(0.0, 0.0);
    let candidates = vec![
        (VoronoiCoord(1), Point2::new(10.0, 0.0)),
        (VoronoiCoord(2), Point2::new(0.0, 10.0)),
    ];
    let history = vec![VoronoiCoord(1), VoronoiCoord(2)];
    let pick = pick_next_voronoi(current, current_centroid, &candidates, &history);
    assert_eq!(pick, None);
}

#[test]
fn pick_next_voronoi_single_candidate_returns_it() {
    let current = VoronoiCoord(0);
    let current_centroid = Point2::new(0.0, 0.0);
    let candidates = vec![
        (VoronoiCoord(1), Point2::new(10.0, 0.0)),
        (VoronoiCoord(2), Point2::new(0.0, 10.0)),
    ];
    let history = vec![VoronoiCoord(2)]; // only V(1) qualifies
    let pick = pick_next_voronoi(current, current_centroid, &candidates, &history);
    assert_eq!(pick, Some(VoronoiCoord(1)));
}

#[test]
fn pick_next_voronoi_closest_centroid_wins() {
    // 3 candidates, all unvisited; pick the geographically-closest.
    let current = VoronoiCoord(0);
    let current_centroid = Point2::new(0.0, 0.0);
    let candidates = vec![
        (VoronoiCoord(1), Point2::new(100.0, 0.0)), // dist 100
        (VoronoiCoord(2), Point2::new(5.0, 0.0)),   // dist 5 ← closest
        (VoronoiCoord(3), Point2::new(20.0, 0.0)),  // dist 20
    ];
    let pick = pick_next_voronoi(current, current_centroid, &candidates, &[]);
    assert_eq!(pick, Some(VoronoiCoord(2)));
}

#[test]
fn pick_next_voronoi_tie_break_lowest_index() {
    // Two candidates equidistant from current; lowest index wins.
    let current = VoronoiCoord(0);
    let current_centroid = Point2::new(0.0, 0.0);
    let candidates = vec![
        (VoronoiCoord(5), Point2::new(10.0, 0.0)),  // dist 10
        (VoronoiCoord(2), Point2::new(0.0, 10.0)),  // dist 10 ← lower idx
        (VoronoiCoord(7), Point2::new(-10.0, 0.0)), // dist 10
    ];
    let pick = pick_next_voronoi(current, current_centroid, &candidates, &[]);
    assert_eq!(pick, Some(VoronoiCoord(2)));
}

#[test]
fn pick_next_voronoi_empty_candidates_returns_none() {
    let pick = pick_next_voronoi(VoronoiCoord(0), Point2::new(0.0, 0.0), &[], &[]);
    assert_eq!(pick, None);
}

#[test]
fn pick_next_voronoi_excludes_current_from_candidates() {
    // C-004 defensive: if Lattice::neighbors mistakenly returns self,
    // pick_next_voronoi must NOT select it.
    let current = VoronoiCoord(0);
    let current_centroid = Point2::new(0.0, 0.0);
    let candidates = vec![
        (current, current_centroid), // self — must be excluded
        (VoronoiCoord(1), Point2::new(50.0, 0.0)),
    ];
    let pick = pick_next_voronoi(current, current_centroid, &candidates, &[]);
    assert_eq!(pick, Some(VoronoiCoord(1)));
}

#[test]
fn pop_out_widget_center_falls_back_to_centroid() {
    let centroid = Point2::new(100.0, 100.0);
    let seed = VoronoiCoord(7);
    let cell_at = |_p: Point2| None; // No candidate satisfies — fallback.
    let result = pop_out_widget_center(centroid, 30.0, seed, cell_at);
    assert_eq!(result, centroid);
}

#[test]
fn seed_handle_alphas_outer_threshold_zero() {
    // Cursor far from every seed → all alphas zero.
    let seeds = vec![egui::pos2(0.0, 0.0), egui::pos2(50.0, 0.0)];
    let alphas = seed_handle_alphas(Some(egui::pos2(1000.0, 1000.0)), &seeds, None);
    assert_eq!(alphas, vec![0u8, 0]);
}

// --- T-010 integration: voronoi_advance_step (history cap + drag-aware) ---

#[test]
fn voronoi_advance_history_caps_at_n_minus_1() {
    // Default 8-seed Voronoi: stepping repeatedly grows history up to
    // capacity 7, then Enter stops (at most after N visits).
    let mut engine = WorkbookEngine::new();
    engine.new_workbook();
    let sid = engine.add_sheet("V", LatticeKind::Voronoi);
    let n = engine.voronoi_lattice(sid).unwrap().len();
    let cap = n.saturating_sub(1);
    let mut history: std::collections::VecDeque<VoronoiCoord> = std::collections::VecDeque::new();
    let mut cursor = VoronoiCoord(0);
    let mut steps = 0;
    // At most N steps before Enter must stop (visit each cell at most once).
    for _ in 0..(n + 2) {
        match voronoi_advance_step(&engine, sid, cursor, &mut history) {
            Some(next) => {
                assert!(
                    history.len() <= cap,
                    "history exceeded cap {cap} at step {steps}"
                );
                cursor = next;
                steps += 1;
            }
            None => break,
        }
    }
    assert!(steps <= n, "Enter visited more than N={n} cells");
    // After exhaustion, one more step must be a no-op (already at None).
    assert_eq!(
        voronoi_advance_step(&engine, sid, cursor, &mut history),
        None,
        "post-exhaustion step must stay None"
    );
}

#[test]
fn voronoi_advance_after_seed_drag_uses_new_centroids() {
    // C-010: dragging a seed mid-traversal must re-evaluate the
    // closest-centroid tie-break against the NEW positions.
    // **C-006 (geometry): keep V(0) off the V(1)-V(2) collinear axis**
    // (push it down 1 px) so the Delaunay triangulation is unambiguous.
    // V(0)=(0,-1) is strictly inside the hull triangle V(1)=(40,0),
    // V(2)=(-40,0), V(3)=(0,40) — all four are connected in the
    // Delaunay graph. From V(0), distances to V(1)/V(2)/V(3) are ≈40
    // each (V(1)/V(2): √(40²+1²)≈40.012; V(3): 41) — V(1) and V(2)
    // are essentially tied with V(3) marginally farther. Tie-break by
    // lowest index → V(1).
    let mut engine = WorkbookEngine::new();
    engine.new_workbook();
    let sid = engine.add_sheet("V", LatticeKind::Voronoi);
    engine
        .set_voronoi_seeds(
            sid,
            vec![[0.0, -1.0], [40.0, 0.0], [-40.0, 0.0], [0.0, 40.0]],
        )
        .unwrap();
    let mut history: std::collections::VecDeque<VoronoiCoord> = std::collections::VecDeque::new();
    let first = voronoi_advance_step(&engine, sid, VoronoiCoord(0), &mut history);
    assert_eq!(first, Some(VoronoiCoord(1)));

    // Drag: move V(1) far right (500, 0) and V(2) close to V(1) at
    // (450, 1). V(0) stays at (0,-1) and V(3) at (0,40). V(2) is
    // interior to the hull triangle V(0)-V(1)-V(3); the y-offsets on
    // V(0) and V(2) keep the triangulation unambiguous.
    // From V(1)=(500,0): V(0)=(0,-1) is in history; V(2)=(450,1) is at
    // dist ≈50; V(3)=(0,40) is at dist ≈501.6. V(2) wins.
    engine
        .set_voronoi_seeds(
            sid,
            vec![[0.0, -1.0], [500.0, 0.0], [450.0, 1.0], [0.0, 40.0]],
        )
        .unwrap();
    let second = voronoi_advance_step(&engine, sid, VoronoiCoord(1), &mut history);
    assert_eq!(
        second,
        Some(VoronoiCoord(2)),
        "post-drag advance must use new centroids; V(2) is now the closest unvisited neighbor of V(1)"
    );
}

#[test]
fn synced_voronoi_lattice_matches_engine_config() {
    // C-005: the load/post-drag resync reflects the engine's stored seeds,
    // not the default — the anti-drift guard for the dual-source split.
    let mut engine = WorkbookEngine::new();
    engine.new_workbook();
    let sid = engine.add_sheet("V", LatticeKind::Voronoi);
    let new = vec![[1.0, 2.0], [30.0, -4.0], [-5.0, 25.0]];
    engine.set_voronoi_seeds(sid, new.clone()).unwrap();
    let lat = synced_voronoi_lattice(&engine, sid);
    let got: Vec<[f32; 2]> = lat.seeds.iter().map(|s| [s.x, s.y]).collect();
    assert_eq!(got, new);
}

#[test]
fn format_number_drops_the_point_for_integers() {
    assert_eq!(format_number(3.0), "3");
    assert_eq!(format_number(-7.0), "-7");
    assert_eq!(format_number(0.0), "0");
}

#[test]
fn format_number_keeps_a_fractional_part() {
    assert_eq!(format_number(3.5), "3.5");
    assert_eq!(format_number(-0.25), "-0.25");
}

#[test]
fn natural_text_renders_each_value_kind() {
    assert_eq!(natural_text(CellValue::Empty), "");
    assert_eq!(natural_text(CellValue::Number(42.0)), "42");
    assert_eq!(natural_text(CellValue::Bool(true)), "TRUE");
    assert_eq!(natural_text(CellValue::Bool(false)), "FALSE");
    assert_eq!(natural_text(CellValue::Text("hi".to_string())), "hi");
}

#[test]
fn error_code_maps_to_excel_style_codes() {
    assert_eq!(error_code(&CellError::DivZero), "#DIV/0!");
    assert_eq!(error_code(&CellError::Ref), "#REF!");
    assert_eq!(error_code(&CellError::Value), "#VALUE!");
    assert_eq!(error_code(&CellError::Num), "#NUM!");
    assert_eq!(error_code(&CellError::Lang("bad token".into())), "#PARSE!");
    // The mapping flows through natural_text.
    assert_eq!(
        natural_text(CellValue::Error(CellError::DivZero)),
        "#DIV/0!",
    );
}

#[test]
fn error_detail_describes_each_error() {
    assert_eq!(error_detail(&CellError::DivZero), "Division by zero");
    assert!(error_detail(&CellError::Ref).contains("missing cell"));
    // Lang / Compile carry the engine's own message.
    assert!(error_detail(&CellError::Lang("unexpected }".into())).contains("unexpected }"));
    assert!(error_detail(&CellError::Compile("E0382".into())).contains("E0382"));
}

#[test]
fn hex_address_formats_axial_coords() {
    assert_eq!(hex_address(HexCoord::new(0, 0)), "H(0,0)");
    assert_eq!(hex_address(HexCoord::new(2, -3)), "H(2,-3)");
    assert_eq!(hex_address(HexCoord::new(-1, 4)), "H(-1,4)");
}

#[test]
fn hex_step_moves_along_the_axial_axes() {
    let origin = HexCoord::new(0, 0);
    assert_eq!(hex_step(origin, Dir::Right), HexCoord::new(1, 0));
    assert_eq!(hex_step(origin, Dir::Left), HexCoord::new(-1, 0));
    assert_eq!(hex_step(origin, Dir::Down), HexCoord::new(0, 1));
    assert_eq!(hex_step(origin, Dir::Up), HexCoord::new(0, -1));
}

#[test]
fn hex_step_opposite_directions_cancel() {
    let c = HexCoord::new(2, -3);
    assert_eq!(hex_step(hex_step(c, Dir::Right), Dir::Left), c);
    assert_eq!(hex_step(hex_step(c, Dir::Down), Dir::Up), c);
}

#[test]
fn hex_step_is_always_a_unit_move() {
    let c = HexCoord::new(1, 1);
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        assert_eq!(c.distance(hex_step(c, dir)), 1);
    }
}

#[test]
fn step_square_clamps_at_the_grid_edges() {
    // The top-left corner can't move further up or left.
    assert_eq!(step_square((0, 0), Dir::Up), (0, 0));
    assert_eq!(step_square((0, 0), Dir::Left), (0, 0));
    // The bottom-right corner can't move further down or right.
    assert_eq!(
        step_square((COLS - 1, ROWS - 1), Dir::Right),
        (COLS - 1, ROWS - 1)
    );
    assert_eq!(
        step_square((COLS - 1, ROWS - 1), Dir::Down),
        (COLS - 1, ROWS - 1)
    );
}

#[test]
fn step_square_moves_within_the_grid() {
    assert_eq!(step_square((3, 4), Dir::Right), (4, 4));
    assert_eq!(step_square((3, 4), Dir::Down), (3, 5));
    assert_eq!(step_square((3, 4), Dir::Left), (2, 4));
    assert_eq!(step_square((3, 4), Dir::Up), (3, 3));
}

#[test]
fn hex_jump_walks_to_a_run_far_end() {
    // A run along the q-axis at r = 0.
    let run = [
        HexCoord::new(0, 0),
        HexCoord::new(1, 0),
        HexCoord::new(2, 0),
    ];
    let target = hex_jump(HexCoord::new(0, 0), Dir::Right, hex_in_view, |c| {
        run.contains(&c)
    });
    // From the run's start, jump to its far end.
    assert_eq!(target, HexCoord::new(2, 0));
}

#[test]
fn hex_jump_skips_a_gap_to_the_next_content() {
    // H(0,0) occupied, a gap, then H(3,0) occupied.
    let content = [HexCoord::new(0, 0), HexCoord::new(3, 0)];
    let target = hex_jump(HexCoord::new(0, 0), Dir::Right, hex_in_view, |c| {
        content.contains(&c)
    });
    assert_eq!(target, HexCoord::new(3, 0));
}

#[test]
fn hex_jump_runs_to_the_view_edge_when_empty() {
    // No content — jump to the last in-view cell along the axis.
    let target = hex_jump(HexCoord::new(0, 0), Dir::Right, hex_in_view, |_| false);
    assert_eq!(target, HexCoord::new(3, 0));
    assert!(hex_in_view(target));
    assert!(!hex_in_view(hex_step(target, Dir::Right)));
}

#[test]
fn hex_jump_at_the_view_edge_stays_put() {
    // H(3,0) is the last in-view cell going right; jumping right stays.
    let target = hex_jump(HexCoord::new(3, 0), Dir::Right, hex_in_view, |_| true);
    assert_eq!(target, HexCoord::new(3, 0));
}

#[test]
fn hex_current_region_grows_to_the_block() {
    // A 2x2 axial block at (0,0)..(1,1).
    let filled = |q, r| (0..=1).contains(&q) && (0..=1).contains(&r);
    assert_eq!(hex_current_region((0, 0), 3, filled), ((0, 0), (1, 1)));
    assert_eq!(hex_current_region((1, 1), 3, filled), ((0, 0), (1, 1)));
}

#[test]
fn hex_current_region_isolated_cell() {
    assert_eq!(
        hex_current_region((0, 0), 3, |_, _| false),
        ((0, 0), (0, 0)),
    );
}

#[test]
fn hex_current_region_clamps_to_the_radius() {
    // Everything occupied — the grow stops at the radius bound.
    assert_eq!(
        hex_current_region((0, 0), 3, |_, _| true),
        ((-3, -3), (3, 3)),
    );
}

#[test]
fn cond_draft_round_trips_through_a_rule() {
    // A threshold rule recovers its kind, threshold, and effects.
    let draft = CondDraft {
        kind: CondKind::Less,
        threshold: "42.5".to_string(),
        bold: true,
        italic: true,
        strikethrough: true,
        underline: true,
        text_color_on: true,
        ..CondDraft::default()
    };
    let back = CondDraft::from_rule(&draft.build().unwrap());
    assert_eq!(back.kind, CondKind::Less);
    assert_eq!(back.threshold, "42.5");
    assert!(back.bold);
    assert!(back.italic && back.strikethrough && back.underline);
    assert!(back.text_color_on);
    // A no-threshold condition recovers its kind.
    let rule = CondDraft {
        kind: CondKind::IsEmpty,
        ..CondDraft::default()
    }
    .build()
    .unwrap();
    assert_eq!(CondDraft::from_rule(&rule).kind, CondKind::IsEmpty);
    // Between round-trips both of its bounds.
    let between = CondDraft {
        kind: CondKind::Between,
        threshold: "10".to_string(),
        threshold2: "20".to_string(),
        ..CondDraft::default()
    };
    let back = CondDraft::from_rule(&between.build().unwrap());
    assert_eq!(back.kind, CondKind::Between);
    assert_eq!(back.threshold, "10");
    assert_eq!(back.threshold2, "20");
}

#[test]
fn hex_autosum_sums_into_the_hex_below() {
    // The column H(0,-1):H(0,1) totals into H(0,2).
    assert_eq!(
        hex_autosum((HexCoord::new(0, -1), HexCoord::new(0, 1)), "SUM"),
        Some((HexCoord::new(0, 2), "=SUM(H(0,-1):H(0,1))".to_string())),
    );
}

#[test]
fn hex_autosum_single_cell_and_chosen_function() {
    assert_eq!(
        hex_autosum((HexCoord::new(1, 0), HexCoord::new(1, 0)), "SUM"),
        Some((HexCoord::new(1, 1), "=SUM(H(1,0))".to_string())),
    );
    // The chosen aggregate name is used verbatim.
    assert_eq!(
        hex_autosum((HexCoord::new(0, -1), HexCoord::new(0, 1)), "AVERAGE"),
        Some((HexCoord::new(0, 2), "=AVERAGE(H(0,-1):H(0,1))".to_string())),
    );
}

#[test]
fn hex_autosum_is_none_outside_the_view() {
    // The total hex would land past the radius-3 view disc.
    assert_eq!(
        hex_autosum((HexCoord::new(0, 2), HexCoord::new(0, 3)), "SUM"),
        None,
    );
}

#[test]
fn triangle_autosum_sums_into_the_triangle_below() {
    // A 3-column × 2-row selection from T(0,0) to T(2,1) totals
    // into T(0,2) — same col as `min`, one row below `max`.
    assert_eq!(
        triangle_autosum((TriCoord::new(0, 0), TriCoord::new(2, 1)), "SUM"),
        Some((TriCoord::new(0, 2), "=SUM(T(0,0):T(2,1))".to_string(),)),
    );
}

#[test]
fn triangle_autosum_single_cell_and_chosen_function() {
    // Single-cell selection → range is just the one address.
    assert_eq!(
        triangle_autosum((TriCoord::new(1, 0), TriCoord::new(1, 0)), "SUM"),
        Some((TriCoord::new(1, 1), "=SUM(T(1,0))".to_string())),
    );
    // The chosen aggregate name is used verbatim.
    assert_eq!(
        triangle_autosum((TriCoord::new(0, 0), TriCoord::new(2, 1)), "AVERAGE"),
        Some((TriCoord::new(0, 2), "=AVERAGE(T(0,0):T(2,1))".to_string(),)),
    );
}

#[test]
fn triangle_autosum_is_none_outside_the_view() {
    // The target triangle would land past the rendered window.
    assert_eq!(
        triangle_autosum(
            (
                TriCoord::new(0, TRIANGLE_RADIUS),
                TriCoord::new(0, TRIANGLE_RADIUS),
            ),
            "SUM",
        ),
        None,
    );
}

#[test]
fn dedup_targets_keeps_the_last_write_per_address() {
    let targets = vec![
        ("A1".to_string(), None),
        ("B1".to_string(), Some("x".to_string())),
        ("A1".to_string(), Some("final".to_string())),
    ];
    let out = dedup_targets(targets);
    assert_eq!(out.len(), 2);
    let a1 = out.iter().find(|(a, _)| a == "A1").unwrap();
    assert_eq!(a1.1, Some("final".to_string()));
}

#[test]
fn dedup_targets_leaves_distinct_addresses_alone() {
    let targets = vec![
        ("A1".to_string(), Some("a".to_string())),
        ("B2".to_string(), None),
    ];
    assert_eq!(dedup_targets(targets).len(), 2);
}

#[test]
fn toggle_target_turns_off_only_when_every_cell_is_on() {
    // Every cell already has the flag — the toggle clears it.
    assert!(!toggle_target([true, true, true].into_iter()));
    // A mix, or all-off — the toggle sets it for the whole range.
    assert!(toggle_target([true, false, true].into_iter()));
    assert!(toggle_target([false, false].into_iter()));
}

#[test]
fn merge_format_edits_keeps_oldest_before_and_newest_after() {
    let cell = (1, 1);
    let red = CellFormat {
        text_color: Some(egui::Color32::RED),
        ..CellFormat::default()
    };
    let blue = CellFormat {
        text_color: Some(egui::Color32::BLUE),
        ..CellFormat::default()
    };
    // Frame 1 changed default -> red; frame 2 changed red -> blue.
    let prev = vec![FormatEdit {
        cell,
        before: CellFormat::default(),
        after: red.clone(),
    }];
    let new = vec![FormatEdit {
        cell,
        before: red,
        after: blue.clone(),
    }];
    let merged = merge_format_edits(prev, new);
    assert_eq!(merged.len(), 1);
    // The merged edit spans the whole gesture: default -> blue.
    assert_eq!(merged[0].before, CellFormat::default());
    assert_eq!(merged[0].after, blue);
}

#[test]
fn merge_hex_format_edits_keeps_oldest_before_and_newest_after() {
    let cell = HexCoord::new(2, -1);
    let red = CellFormat {
        text_color: Some(egui::Color32::RED),
        ..CellFormat::default()
    };
    let blue = CellFormat {
        text_color: Some(egui::Color32::BLUE),
        ..CellFormat::default()
    };
    let prev = vec![HexFormatEdit {
        cell,
        before: CellFormat::default(),
        after: red.clone(),
    }];
    let new = vec![HexFormatEdit {
        cell,
        before: red,
        after: blue.clone(),
    }];
    let merged = merge_hex_format_edits(prev, new);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].before, CellFormat::default());
    assert_eq!(merged[0].after, blue);
}

#[test]
fn hex_text_layout_anchors_by_alignment() {
    let c = egui::pos2(100.0, 50.0);
    let (anchor, pos) = hex_text_layout(HAlign::Center, c, 10.0);
    assert_eq!(anchor, egui::Align2::CENTER_CENTER);
    assert_eq!(pos, c);
    let (anchor, pos) = hex_text_layout(HAlign::Left, c, 10.0);
    assert_eq!(anchor, egui::Align2::LEFT_CENTER);
    assert_eq!(pos, egui::pos2(90.0, 50.0));
    let (anchor, pos) = hex_text_layout(HAlign::Right, c, 10.0);
    assert_eq!(anchor, egui::Align2::RIGHT_CENTER);
    assert_eq!(pos, egui::pos2(110.0, 50.0));
}

#[test]
fn commit_source_clears_on_blank_keeps_content() {
    assert_eq!(
        commit_source("=SUM(A1:A3)"),
        Some("=SUM(A1:A3)".to_string())
    );
    assert_eq!(commit_source("42"), Some("42".to_string()));
    // Empty or all-whitespace clears the cell.
    assert_eq!(commit_source(""), None);
    assert_eq!(commit_source("   "), None);
    // Content with surrounding spaces is kept verbatim.
    assert_eq!(commit_source("  hi  "), Some("  hi  ".to_string()));
}
