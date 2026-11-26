//! Pure drawing, navigation, formatting, and edit-coalescing helpers shared
//! across `CarbideApp` submodules.

use carbide_core::{CellError, CellValue, EngineKind, SheetId};
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use eframe::egui;

use crate::conditional::Condition;
use crate::format::{self, Borders, CellFormat, HAlign};
use crate::grid;
use crate::keymap::Dir;

use super::types::FormatEdit;
use super::{COLS, HEX_VIEW_RADIUS, ROWS, TRIANGLE_RADIUS};

/// Draw a square cell's value with its formatting — colour, alignment,
/// italic, and faux-bold (a second offset pass, since egui's default font
/// has no bold weight).
/// A small triangle in a cell's top-right corner, marking that the cell
/// carries a note.
pub(super) fn draw_note_marker(painter: &egui::Painter, rect: egui::Rect) {
    let size = 7.0;
    let tr = rect.right_top();
    let marker = vec![
        egui::pos2(tr.x - size, tr.y),
        egui::pos2(tr.x, tr.y),
        egui::pos2(tr.x, tr.y + size),
    ];
    painter.add(egui::Shape::convex_polygon(
        marker,
        egui::Color32::from_rgb(220, 90, 70),
        egui::Stroke::NONE,
    ));
}

pub(super) fn draw_cell_text(
    painter: &egui::Painter,
    text: &str,
    rect: egui::Rect,
    fmt: &CellFormat,
    numeric: bool,
    is_negative: bool,
    default_color: egui::Color32,
) {
    let color = format::effective_text_color(fmt, is_negative, default_color);
    let pad = 5.0;
    let mut job = egui::text::LayoutJob::default();
    if fmt.wrap_text {
        job.wrap.max_width = (rect.width() - 2.0 * pad).max(0.0);
    }
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(fmt.font_size.points()),
            color,
            italics: fmt.italic,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let size = galley.size();
    let y = rect.top() + format::vertical_offset(fmt.valign, rect.height(), size.y, pad);
    let x = match format::effective_align(fmt.align, numeric) {
        HAlign::Left | HAlign::Auto => rect.left() + pad,
        HAlign::Center => rect.center().x - size.x / 2.0,
        HAlign::Right => rect.right() - pad - size.x,
    };
    let pos = egui::pos2(x, y);
    painter.galley(pos, galley.clone(), color);
    if fmt.bold {
        painter.galley(pos + egui::vec2(0.55, 0.0), galley, color);
    }
    draw_text_decorations(painter, egui::Rect::from_min_size(pos, size), fmt, color);
}

/// Draw the strikethrough / underline lines for `fmt` across a text's
/// bounding `rect`, in `color`. A no-op when neither decoration is set —
/// shared by the square and hex cell-text drawing.
pub(super) fn draw_text_decorations(
    painter: &egui::Painter,
    rect: egui::Rect,
    fmt: &CellFormat,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0_f32, color);
    if fmt.strikethrough {
        painter.hline(rect.x_range(), rect.center().y, stroke);
    }
    if fmt.underline {
        painter.hline(rect.x_range(), rect.bottom() - 1.0, stroke);
    }
}

/// One step from a square cell, clamped to the grid.
pub(super) fn step_square(cell: (u32, u32), dir: Dir) -> (u32, u32) {
    let (col, row) = cell;
    match dir {
        Dir::Up => (col, row.saturating_sub(1)),
        Dir::Down => (col, (row + 1).min(ROWS - 1)),
        Dir::Left => (col.saturating_sub(1), row),
        Dir::Right => ((col + 1).min(COLS - 1), row),
    }
}

/// One axial step from a hex coord. On the pointy-top lattice, left/right
/// run along the q-axis (pure horizontal on screen) and up/down along the
/// r-axis. Any combination reaches every hex.
pub(super) fn hex_step(coord: HexCoord, dir: Dir) -> HexCoord {
    match dir {
        Dir::Left => HexCoord::new(coord.q - 1, coord.r),
        Dir::Right => HexCoord::new(coord.q + 1, coord.r),
        Dir::Up => HexCoord::new(coord.q, coord.r - 1),
        Dir::Down => HexCoord::new(coord.q, coord.r + 1),
    }
}

/// Whether a hex coord lies within the rendered radius-3 view disc.
pub(super) fn hex_in_view(coord: HexCoord) -> bool {
    HexCoord::new(0, 0).distance(coord) <= HEX_VIEW_RADIUS
}

/// Step a triangle coord one cell along the keyboard direction. `col`
/// is the horizontal half-base index, `row` the vertical row — left /
/// right move along `col`, up / down along `row`. The triangle
/// orientation is implicit in the parity of `col + row`, so a single
/// `Right` step naturally toggles between `△` and `▽`.
pub(super) fn triangle_step(coord: TriCoord, dir: Dir) -> TriCoord {
    match dir {
        Dir::Left => TriCoord::new(coord.col - 1, coord.row),
        Dir::Right => TriCoord::new(coord.col + 1, coord.row),
        Dir::Up => TriCoord::new(coord.col, coord.row - 1),
        Dir::Down => TriCoord::new(coord.col, coord.row + 1),
    }
}

/// The hex cell a Ctrl+arrow jump lands on — [`grid::block_jump`] walking
/// `dir` from `start` over axial coordinates, bounded by `in_view`.
pub(super) fn hex_jump(
    start: HexCoord,
    dir: Dir,
    in_view: impl Fn(HexCoord) -> bool,
    occupied: impl Fn(HexCoord) -> bool,
) -> HexCoord {
    let step = |c: HexCoord| {
        let n = hex_step(c, dir);
        in_view(n).then_some(n)
    };
    grid::block_jump(start, step, occupied)
}

/// The hex AutoSum action for a selection whose inclusive axial corners
/// are `bounds`: the hex directly below the selection's bottom row that
/// should hold the result, and the `=FUNC(...)` formula (`func` is an
/// aggregate name). `None` when that hex falls outside the visible disc.
pub(super) fn hex_autosum(bounds: (HexCoord, HexCoord), func: &str) -> Option<(HexCoord, String)> {
    let (min, max) = bounds;
    let (min_q, min_r, max_q, max_r) = (min.q, min.r, max.q, max.r);
    let target = HexCoord::new(min_q, max_r + 1);
    if !hex_in_view(target) {
        return None;
    }
    let range = if (min_q, min_r) == (max_q, max_r) {
        hex_address(HexCoord::new(min_q, min_r))
    } else {
        format!(
            "{}:{}",
            hex_address(HexCoord::new(min_q, min_r)),
            hex_address(HexCoord::new(max_q, max_r)),
        )
    };
    Some((target, format!("={func}({range})")))
}

/// The triangle AutoSum action for a selection between `min` and `max`:
/// the triangle directly below the selection's bottom row (one row
/// further along `row`, sharing the cursor's column) that should hold
/// the result, and the `=FUNC(...)` formula (`func` is an aggregate
/// name). `None` when that triangle would fall outside the rendered
/// window.
pub(super) fn triangle_autosum(
    bounds: (TriCoord, TriCoord),
    func: &str,
) -> Option<(TriCoord, String)> {
    let (min, max) = bounds;
    let target = TriCoord::new(min.col, max.row + 1);
    if !triangle_in_view(target) {
        return None;
    }
    let range = if min == max {
        triangle_address(min)
    } else {
        format!("{}:{}", triangle_address(min), triangle_address(max))
    };
    Some((target, format!("={func}({range})")))
}

/// The current hex data region around `cursor` — the axial analogue of
/// [`grid::current_region`]. The `(q, r)` box grows outward to a
/// fixpoint while the row or column just past an edge holds an
/// `occupied` cell within the box's span; `occupied` reports `false`
/// for out-of-view cells, so the grow halts at the disc. `radius`
/// bounds the walk.
pub(super) fn hex_current_region(
    cursor: (i32, i32),
    radius: i32,
    occupied: impl Fn(i32, i32) -> bool,
) -> ((i32, i32), (i32, i32)) {
    let (mut min_q, mut min_r) = cursor;
    let (mut max_q, mut max_r) = cursor;
    loop {
        let mut grew = false;
        if min_r > -radius && (min_q..=max_q).any(|q| occupied(q, min_r - 1)) {
            min_r -= 1;
            grew = true;
        }
        if max_r < radius && (min_q..=max_q).any(|q| occupied(q, max_r + 1)) {
            max_r += 1;
            grew = true;
        }
        if min_q > -radius && (min_r..=max_r).any(|r| occupied(min_q - 1, r)) {
            min_q -= 1;
            grew = true;
        }
        if max_q < radius && (min_r..=max_r).any(|r| occupied(max_q + 1, r)) {
            max_q += 1;
            grew = true;
        }
        if !grew {
            return ((min_q, min_r), (max_q, max_r));
        }
    }
}

/// Render a cell value with the engine's natural formatting — no number
/// format applied. Used for the hex sheet and as the square sheet's
/// fallback for non-numeric values.
/// A friendly description of a formula error — shown as a tooltip when
/// an errored cell is hovered. `Lang` / `Compile` carry the engine's
/// own message.
pub(super) fn error_detail(err: &CellError) -> String {
    match err {
        CellError::Ref => "Reference to a missing cell".to_string(),
        CellError::Cycle => "Part of a dependency cycle".to_string(),
        CellError::DivZero => "Division by zero".to_string(),
        CellError::Num => "Numeric overflow or invalid number".to_string(),
        CellError::Value => "Wrong type of value".to_string(),
        CellError::Lang(msg) => format!("Formula error: {msg}"),
        CellError::Compile(msg) => format!("Compile error: {msg}"),
        CellError::Timeout => "Formula timed out".to_string(),
        CellError::Spill => "Spill range is blocked".to_string(),
        CellError::StaleFunction => "Stored function needs recalculation".to_string(),
    }
}

/// The Excel-style short code for a formula error — what an errored
/// cell displays.
pub(super) fn error_code(err: &CellError) -> &'static str {
    match err {
        CellError::Ref => "#REF!",
        CellError::Cycle => "#CYCLE!",
        CellError::DivZero => "#DIV/0!",
        CellError::Num => "#NUM!",
        CellError::Value => "#VALUE!",
        CellError::Lang(_) => "#PARSE!",
        CellError::Compile(_) => "#COMPILE!",
        CellError::Timeout => "#TIMEOUT!",
        CellError::Spill => "#SPILL!",
        CellError::StaleFunction => "#STALE!",
    }
}

pub(super) fn natural_text(value: CellValue) -> String {
    match value {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => format_number(n),
        CellValue::Integer(i) => i.to_string(),
        CellValue::Bool(b) => if b { "TRUE" } else { "FALSE" }.to_string(),
        CellValue::Text(t) => t,
        CellValue::Error(e) => error_code(&e).to_string(),
        CellValue::Array(_) => "{array}".to_string(),
        _ => String::new(),
    }
}

/// A short human description of a condition, for the rule editor's list.
pub(super) fn describe_condition(condition: &Condition) -> String {
    match condition {
        Condition::GreaterThan(t) => format!("value > {}", format_number(*t)),
        Condition::GreaterOrEqual(t) => format!("value >= {}", format_number(*t)),
        Condition::LessThan(t) => format!("value < {}", format_number(*t)),
        Condition::LessOrEqual(t) => format!("value <= {}", format_number(*t)),
        Condition::EqualTo(t) => format!("value = {}", format_number(*t)),
        Condition::NotEqualTo(t) => format!("value != {}", format_number(*t)),
        Condition::Between(a, b) => format!(
            "value in {}..{}",
            format_number(a.min(*b)),
            format_number(a.max(*b)),
        ),
        Condition::Contains(s) => format!("text contains \"{s}\""),
        Condition::IsTrue => "value is TRUE".to_string(),
        Condition::IsFalse => "value is FALSE".to_string(),
        Condition::NonEmpty => "cell is non-empty".to_string(),
        Condition::IsEmpty => "cell is empty".to_string(),
    }
}

/// The source to write for an edit buffer — `None` (a cleared cell) when
/// the buffer is empty or all whitespace, otherwise the buffer verbatim.
/// Shared by the in-cell editor and the formula bar.
pub(super) fn commit_source(buffer: &str) -> Option<String> {
    if buffer.trim().is_empty() {
        None
    } else {
        Some(buffer.to_string())
    }
}

/// Draw a cell's set border sides as line segments over the grid line.
pub(super) fn draw_borders(
    painter: &egui::Painter,
    rect: egui::Rect,
    borders: &Borders,
    stroke: egui::Stroke,
) {
    if borders.top {
        painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    }
    if borders.bottom {
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    }
    if borders.left {
        painter.line_segment([rect.left_top(), rect.left_bottom()], stroke);
    }
    if borders.right {
        painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    }
}

/// The anchor and position to draw a hex cell's text at, given its
/// horizontal alignment. Left/Right anchor `inset` points either side of
/// the `centroid`; Center sits on it.
pub(super) fn hex_text_layout(
    align: HAlign,
    centroid: egui::Pos2,
    inset: f32,
) -> (egui::Align2, egui::Pos2) {
    match align {
        HAlign::Left | HAlign::Auto => (
            egui::Align2::LEFT_CENTER,
            egui::pos2(centroid.x - inset, centroid.y),
        ),
        HAlign::Center => (egui::Align2::CENTER_CENTER, centroid),
        HAlign::Right => (
            egui::Align2::RIGHT_CENTER,
            egui::pos2(centroid.x + inset, centroid.y),
        ),
    }
}

/// Format a numeric cell value: integers without a fractional part, other
/// finite numbers with Rust's default float formatting.
pub(super) fn format_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The `H(q,r)` address string for a hex coord — the form the engine's
/// hex lattice canonicalizes to.
pub(super) fn hex_address(c: HexCoord) -> String {
    format!("H({},{})", c.q, c.r)
}

/// The `T(col,row)` address string for a triangle coord — the form the
/// engine's triangle lattice canonicalizes to.
pub(super) fn triangle_address(c: TriCoord) -> String {
    format!("T({},{})", c.col, c.row)
}

/// Whether `coord` is inside the currently-drawn triangle window.
/// Mirrors `hex_in_view` — render and hit-test agree on the same bound.
pub(super) fn triangle_in_view(c: TriCoord) -> bool {
    c.col.abs() <= TRIANGLE_RADIUS && c.row.abs() <= TRIANGLE_RADIUS
}

/// Alpha-blend `over` onto `under` with the standard source-over rule.
/// Used for selection tinting: the translucent selection colour sits on
/// top of the cell's base fill (which may itself be transparent).
pub(super) fn blend_over(over: egui::Color32, under: egui::Color32) -> egui::Color32 {
    let oa = over.a() as f32 / 255.0;
    let ua = under.a() as f32 / 255.0;
    let out_a = oa + ua * (1.0 - oa);
    if out_a <= 0.0 {
        return egui::Color32::TRANSPARENT;
    }
    let blend = |o: u8, u: u8| -> u8 {
        let o = o as f32;
        let u = u as f32;
        let v = (o * oa + u * ua * (1.0 - oa)) / out_a;
        v.round().clamp(0.0, 255.0) as u8
    };
    egui::Color32::from_rgba_unmultiplied(
        blend(over.r(), under.r()),
        blend(over.g(), under.g()),
        blend(over.b(), under.b()),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Human label for an [`EngineKind`] in the formula-bar language
/// picker. When `is_override` is true (i.e. the cell has an explicit
/// engine override rather than inheriting the workbook default), the
/// label is shown verbatim; otherwise a trailing `(default)` hint
/// signals that the choice was inherited.
pub(super) fn engine_label(kind: EngineKind, is_override: bool) -> String {
    let base = match kind {
        // The built-in formula language is "Carbide" — `ExcelLite` is the
        // historical enum name kept for serialization back-compat.
        EngineKind::ExcelLite => "Carbide",
        EngineKind::Python => "Python",
        EngineKind::Rhai => "Rhai",
        EngineKind::RustNative => "Rust",
    };
    if is_override {
        base.to_string()
    } else {
        format!("{base} (default)")
    }
}

/// Picker selected-text for the formula-bar language ComboBox. The
/// closed combo shows the cell's *intent*: a `None` override means
/// "Default ({base})" because the cell follows the workbook default;
/// an explicit override shows that engine's name verbatim.
pub(super) fn picker_label(
    cell_override: Option<EngineKind>,
    default_engine: EngineKind,
) -> String {
    match cell_override {
        None => format!("Default ({})", engine_label(default_engine, true)),
        Some(kind) => engine_label(kind, true),
    }
}

/// Collapse duplicate addresses in an edit batch so one undo step never
/// holds two edits for the same cell — the last write for an address
/// wins (e.g. a paste landing on a just-cleared cut cell).
pub(super) fn dedup_targets(
    targets: Vec<(String, Option<String>)>,
) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for (addr, after) in targets {
        if let Some(slot) = out.iter_mut().find(|(a, _)| *a == addr) {
            slot.1 = after;
        } else {
            out.push((addr, after));
        }
    }
    out
}

/// Collapse duplicate `(sheet, address)` pairs in a multi-sheet edit batch
/// so one atomic undo step never holds two edits for the same cell — the
/// last write wins (e.g. a paste overwriting a just-cleared cut origin).
pub(super) fn dedup_sheet_targets(
    targets: Vec<(SheetId, String, Option<String>)>,
) -> Vec<(SheetId, String, Option<String>)> {
    let mut out: Vec<(SheetId, String, Option<String>)> = Vec::new();
    for (sheet, addr, after) in targets {
        if let Some(slot) = out.iter_mut().find(|(s, a, _)| *s == sheet && *a == addr) {
            slot.2 = after;
        } else {
            out.push((sheet, addr, after));
        }
    }
    out
}

/// The new state for a range toggle: off when every cell already has the
/// flag, on otherwise — Excel's bold/italic-over-a-range rule.
pub(super) fn toggle_target(mut currently_on: impl Iterator<Item = bool>) -> bool {
    !currently_on.all(|on| on)
}

/// Merge two consecutive format-edit batches over the same cells into
/// one — the earliest `before` and the latest `after` per cell, so a
/// coalesced gesture undoes to its true starting state across any lattice.
pub(super) fn merge_format_edits(prev: Vec<FormatEdit>, new: Vec<FormatEdit>) -> Vec<FormatEdit> {
    new.into_iter()
        .map(|n| {
            let before = prev
                .iter()
                .find(|p| p.cell == n.cell)
                .map(|p| p.before.clone())
                .unwrap_or_else(|| n.before.clone());
            FormatEdit {
                cell: n.cell,
                before,
                after: n.after,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_sheet_targets_keeps_last_write_per_sheet_and_preserves_distinct_sheets() {
        let s1 = SheetId(1);
        let s2 = SheetId(2);
        let targets = vec![
            (s1, "A1".to_string(), None),
            (s2, "A1".to_string(), Some("10".to_string())),
            (s1, "A1".to_string(), Some("override".to_string())),
        ];
        let out = dedup_sheet_targets(targets);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], (s1, "A1".to_string(), Some("override".to_string())));
        assert_eq!(out[1], (s2, "A1".to_string(), Some("10".to_string())));
    }
}
