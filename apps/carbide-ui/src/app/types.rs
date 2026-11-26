//! Internal UI state, edit-history, and conditional-draft types for `CarbideApp`.

use carbide_core::SheetId;
use carbide_tess::hex::HexCoord;
use carbide_tess::triangle::TriCoord;
use carbide_tess::voronoi::VoronoiCoord;
use eframe::egui;

use crate::conditional::{Condition, Rule};
use crate::format::CellFormat;

/// Which of the workbook's sheets is currently on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActiveSheet {
    Square,
    Hex,
    Triangle,
    Voronoi,
}

/// A cell on any sheet — Find tracks matches across every lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CellId {
    Square((u32, u32)),
    Hex(HexCoord),
    Triangle(TriCoord),
    Voronoi(VoronoiCoord),
}

/// An in-progress cell edit.
pub(super) struct EditState {
    pub(super) buffer: String,
    /// Set on the first frame so the `TextEdit` can grab focus and put the
    /// caret at the end; cleared once that has happened.
    pub(super) fresh: bool,
}

/// A header border being dragged to resize.
#[derive(Debug, Clone, Copy)]
pub(super) enum Resize {
    Column(u32),
    Row(u32),
}

/// A column or row header being dragged to sweep a range selection. The
/// index is the column/row where the drag began.
#[derive(Debug, Clone, Copy)]
pub(super) enum HeaderDrag {
    Column(u32),
    Row(u32),
}

/// Which axis a fill-handle drag is extending the selection along.
/// The axis is locked on the first non-trivial drag movement and
/// stays fixed for the rest of the drag, so the extension is one
/// dimensional even if the user wobbles diagonally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FillAxis {
    Down,
    Right,
}

/// In-progress fill-handle drag state. `original` is the selection's
/// bounds when the drag began; `axis` is `None` until the user has
/// moved far enough for the drag direction to be unambiguous, then it
/// stays locked for the rest of the drag.
#[derive(Debug, Clone, Copy)]
pub(super) struct FillDrag {
    pub(super) original: ((u32, u32), (u32, u32)),
    pub(super) axis: Option<FillAxis>,
}

/// The kind of condition picked in the conditional-formatting editor — a
/// [`Condition`] without its threshold, which the editor's text field
/// supplies separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CondKind {
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Equal,
    NotEqual,
    Between,
    Contains,
    IsTrue,
    IsFalse,
    NonEmpty,
    IsEmpty,
}

impl CondKind {
    pub(super) const ALL: [CondKind; 12] = [
        CondKind::Greater,
        CondKind::GreaterEqual,
        CondKind::Less,
        CondKind::LessEqual,
        CondKind::Equal,
        CondKind::NotEqual,
        CondKind::Between,
        CondKind::Contains,
        CondKind::IsTrue,
        CondKind::IsFalse,
        CondKind::NonEmpty,
        CondKind::IsEmpty,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            CondKind::Greater => "greater than",
            CondKind::GreaterEqual => "greater or equal",
            CondKind::Less => "less than",
            CondKind::LessEqual => "less or equal",
            CondKind::Equal => "equal to",
            CondKind::NotEqual => "not equal to",
            CondKind::Between => "between",
            CondKind::Contains => "contains text",
            CondKind::IsTrue => "is TRUE",
            CondKind::IsFalse => "is FALSE",
            CondKind::NonEmpty => "non-empty",
            CondKind::IsEmpty => "empty",
        }
    }

    /// Whether this kind needs the first numeric threshold field.
    pub(super) fn needs_threshold(self) -> bool {
        matches!(
            self,
            CondKind::Greater
                | CondKind::GreaterEqual
                | CondKind::Less
                | CondKind::LessEqual
                | CondKind::Equal
                | CondKind::NotEqual
                | CondKind::Between
                | CondKind::Contains
        )
    }

    /// Whether this kind needs the second numeric threshold field — only
    /// `Between`, whose range carries an upper bound.
    pub(super) fn needs_threshold2(self) -> bool {
        matches!(self, CondKind::Between)
    }

    /// The kind matching an existing condition — the inverse of the
    /// `kind` arm of [`CondDraft::build`].
    pub(super) fn from_condition(condition: &Condition) -> CondKind {
        match condition {
            Condition::GreaterThan(_) => CondKind::Greater,
            Condition::GreaterOrEqual(_) => CondKind::GreaterEqual,
            Condition::LessThan(_) => CondKind::Less,
            Condition::LessOrEqual(_) => CondKind::LessEqual,
            Condition::EqualTo(_) => CondKind::Equal,
            Condition::NotEqualTo(_) => CondKind::NotEqual,
            Condition::Between(..) => CondKind::Between,
            Condition::Contains(_) => CondKind::Contains,
            Condition::IsTrue => CondKind::IsTrue,
            Condition::IsFalse => CondKind::IsFalse,
            Condition::NonEmpty => CondKind::NonEmpty,
            Condition::IsEmpty => CondKind::IsEmpty,
        }
    }
}

/// The in-progress new rule in the conditional-formatting editor.
pub(super) struct CondDraft {
    pub(super) kind: CondKind,
    pub(super) threshold: String,
    /// The range's upper bound for `Between`; unused by other kinds.
    pub(super) threshold2: String,
    pub(super) fill: egui::Color32,
    /// Whether a matching cell is also rendered bold.
    pub(super) bold: bool,
    /// Whether a matching cell is also rendered italic.
    pub(super) italic: bool,
    /// Whether a matching cell is also struck through.
    pub(super) strikethrough: bool,
    /// Whether a matching cell is also underlined.
    pub(super) underline: bool,
    /// Whether a matching cell's text is recoloured, and to what.
    pub(super) text_color_on: bool,
    pub(super) text_color: egui::Color32,
}

impl Default for CondDraft {
    fn default() -> Self {
        Self {
            kind: CondKind::Greater,
            threshold: "100".to_string(),
            threshold2: "200".to_string(),
            fill: egui::Color32::from_rgb(255, 235, 156),
            bold: false,
            italic: false,
            strikethrough: false,
            underline: false,
            text_color_on: false,
            text_color: egui::Color32::from_rgb(190, 40, 40),
        }
    }
}

impl CondDraft {
    /// Build a [`Rule`] from the draft, or `None` when a numeric
    /// threshold is required but the field doesn't parse.
    pub(super) fn build(&self) -> Option<Rule> {
        let condition = match self.kind {
            CondKind::Greater => Condition::GreaterThan(self.threshold.trim().parse().ok()?),
            CondKind::GreaterEqual => {
                Condition::GreaterOrEqual(self.threshold.trim().parse().ok()?)
            }
            CondKind::Less => Condition::LessThan(self.threshold.trim().parse().ok()?),
            CondKind::LessEqual => Condition::LessOrEqual(self.threshold.trim().parse().ok()?),
            CondKind::Equal => Condition::EqualTo(self.threshold.trim().parse().ok()?),
            CondKind::NotEqual => Condition::NotEqualTo(self.threshold.trim().parse().ok()?),
            CondKind::Between => Condition::Between(
                self.threshold.trim().parse().ok()?,
                self.threshold2.trim().parse().ok()?,
            ),
            CondKind::Contains => {
                let needle = self.threshold.trim();
                if needle.is_empty() {
                    return None;
                }
                Condition::Contains(needle.to_string())
            }
            CondKind::IsTrue => Condition::IsTrue,
            CondKind::IsFalse => Condition::IsFalse,
            CondKind::NonEmpty => Condition::NonEmpty,
            CondKind::IsEmpty => Condition::IsEmpty,
        };
        Some(Rule {
            condition,
            format: CellFormat {
                fill: Some(self.fill),
                bold: self.bold,
                italic: self.italic,
                strikethrough: self.strikethrough,
                underline: self.underline,
                text_color: self.text_color_on.then_some(self.text_color),
                ..CellFormat::default()
            },
        })
    }

    /// Load an existing [`Rule`] back into a draft — the inverse of
    /// [`CondDraft::build`], for the editor's "Edit" button.
    pub(super) fn from_rule(rule: &Rule) -> Self {
        let defaults = CondDraft::default();
        let threshold = match &rule.condition {
            Condition::GreaterThan(t)
            | Condition::LessThan(t)
            | Condition::EqualTo(t)
            | Condition::NotEqualTo(t)
            | Condition::GreaterOrEqual(t)
            | Condition::LessOrEqual(t)
            | Condition::Between(t, _) => t.to_string(),
            Condition::Contains(s) => s.clone(),
            _ => defaults.threshold,
        };
        let threshold2 = match &rule.condition {
            Condition::Between(_, t) => t.to_string(),
            _ => defaults.threshold2,
        };
        CondDraft {
            kind: CondKind::from_condition(&rule.condition),
            threshold,
            threshold2,
            fill: rule.format.fill.unwrap_or(defaults.fill),
            bold: rule.format.bold,
            italic: rule.format.italic,
            strikethrough: rule.format.strikethrough,
            underline: rule.format.underline,
            text_color_on: rule.format.text_color.is_some(),
            text_color: rule.format.text_color.unwrap_or(defaults.text_color),
        }
    }
}

/// One cell's source before and after a change.
#[derive(Debug, Clone)]
pub(super) struct CellEdit {
    pub(super) sheet: SheetId,
    pub(super) addr: String,
    pub(super) before: Option<String>,
    pub(super) after: Option<String>,
}

/// One cell's format before and after a change, on any lattice.
#[derive(Debug, Clone)]
pub(super) struct FormatEdit {
    pub(super) cell: CellId,
    pub(super) before: CellFormat,
    pub(super) after: CellFormat,
}

/// An undoable action — a batch of cell-content edits, or a batch of
/// formatting edits on any lattice. One `Action` is one undo step.
#[derive(Debug, Clone)]
pub(super) enum Action {
    Cells(Vec<CellEdit>),
    Formats(Vec<FormatEdit>),
}
