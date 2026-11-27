//! Byte-driven spreadsheet importers (`.csv` and `.xlsx`).
//!
//! All entrypoints take `&[u8]` so desktop native, headless CLI, and
//! WebAssembly targets share one implementation. Imported workbooks use
//! `LatticeKind::Square` sheets with canonical A1 addresses (`carbide-tess`),
//! preserving `=`-prefixed formulas so `WorkbookEngine` can build the DAG and
//! evaluate them immediately.

use carbide_core::{
    Cell, CellValue, EngineKind, Sheet, SheetExtent, SheetId, Workbook, WorkbookId, WorkbookMeta,
};
use carbide_tess::square::{SquareCoord, SquareLattice};
use carbide_tess::{Lattice, LatticeKind};
use hashbrown::HashMap;

use crate::{StoreError, FORMAT_VERSION};

/// Supported external spreadsheet formats for import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Csv,
    Xlsx,
}

/// Detect whether an import payload is `.xlsx` or `.csv`.
///
/// Inspects the file extension (case-insensitive) first when available,
/// falling back to the ZIP local-file-header magic bytes (`PK\x03\x04`) for
/// `.xlsx` payloads when no recognized extension is supplied.
pub fn detect_import_format(filename: Option<&str>, bytes: &[u8]) -> ImportFormat {
    if let Some(name) = filename {
        let lower = name.trim().to_ascii_lowercase();
        if lower.ends_with(".xlsx") {
            return ImportFormat::Xlsx;
        }
        if lower.ends_with(".csv") {
            return ImportFormat::Csv;
        }
    }
    if bytes.starts_with(b"PK\x03\x04") {
        ImportFormat::Xlsx
    } else {
        ImportFormat::Csv
    }
}

/// Derive a human-friendly sheet name from a file path or name (stripping
/// POSIX `/` and Windows `\` directory components and the final extension).
/// Falls back to `"Sheet1"` when `filename` is absent, empty, or has an empty
/// stem (such as `".csv"`).
pub fn sheet_name_from_filename(filename: Option<&str>) -> String {
    let Some(raw) = filename else {
        return "Sheet1".to_string();
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Sheet1".to_string();
    }
    let base = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).trim();
    if base.is_empty() {
        return "Sheet1".to_string();
    }
    let stem = match base.rfind('.') {
        Some(0) => "",
        Some(dot) => &base[..dot],
        None => base,
    }
    .trim();
    if stem.is_empty() {
        "Sheet1".to_string()
    } else {
        stem.to_string()
    }
}

/// Parse a literal CSV field into a typed [`CellValue`], matching
/// `carbide-formula`'s `parse_literal` rules (integers, finite floats,
/// case-insensitive booleans, else text). Non-finite strings like `"NaN"` or
/// `"inf"` stay [`CellValue::Text`] so `.crbd` JSON round-trips stay valid.
fn parse_csv_literal(raw: &str) -> CellValue {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("true") {
        return CellValue::Bool(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return CellValue::Bool(false);
    }
    if let Ok(i) = t.parse::<i64>() {
        return CellValue::Integer(i);
    }
    if let Ok(n) = t.parse::<f64>() {
        if n.is_finite() {
            return CellValue::Number(n);
        }
    }
    CellValue::Text(raw.to_string())
}

/// Parse UTF-8 CSV bytes into a single-sheet `LatticeKind::Square` [`Workbook`].
///
/// - Strips an optional leading UTF-8 BOM (`0xEF 0xBB 0xBF`).
/// - Treats row 1 as spreadsheet row 1 (`has_headers(false)`) and allows
///   ragged rows (`flexible(true)`).
/// - Skips blank/empty fields to keep the sheet sparse.
/// - Preserves `=`-prefixed fields as formula sources (`CellValue::Empty`
///   until evaluated by `WorkbookEngine`); all other non-empty fields are
///   inferred as typed literals.
pub fn import_csv_bytes(bytes: &[u8], sheet_name: &str) -> Result<Workbook, StoreError> {
    let payload = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(payload);

    let lattice = SquareLattice::default();
    let mut cells: HashMap<String, Cell> = HashMap::new();

    for (row_idx, record) in reader.records().enumerate() {
        let record = record?;
        let row = i32::try_from(row_idx)
            .map_err(|_| StoreError::Format("csv exceeds maximum row index".into()))?;
        for (col_idx, field) in record.iter().enumerate() {
            if field.trim().is_empty() {
                continue;
            }
            let col = i32::try_from(col_idx)
                .map_err(|_| StoreError::Format("csv exceeds maximum column index".into()))?;
            let addr = lattice.address(SquareCoord { col, row });
            let cell = if field.trim_start().starts_with('=') {
                Cell {
                    source: Some(field.to_string()),
                    engine: None,
                    value: CellValue::Empty,
                }
            } else {
                Cell {
                    source: Some(field.to_string()),
                    engine: None,
                    value: parse_csv_literal(field),
                }
            };
            cells.insert(addr, cell);
        }
    }

    let name = if sheet_name.trim().is_empty() {
        "Sheet1".to_string()
    } else {
        sheet_name.trim().to_string()
    };
    let sid = SheetId(1);
    let mut sheets = HashMap::new();
    sheets.insert(
        sid,
        Sheet {
            id: sid,
            name: name.clone(),
            lattice: LatticeKind::Square,
            extent: SheetExtent::Unbounded,
            lattice_config: None,
            cells,
        },
    );

    Ok(Workbook {
        id: WorkbookId(1),
        meta: WorkbookMeta {
            title: name,
            created_at: String::new(),
            format_version: FORMAT_VERSION,
        },
        default_engine: EngineKind::ExcelLite,
        sheet_order: vec![sid],
        sheets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_import_format_checks_extension_and_magic_bytes() {
        assert_eq!(
            detect_import_format(Some("Budget.XLSX"), b"not-a-zip"),
            ImportFormat::Xlsx
        );
        assert_eq!(
            detect_import_format(Some("data.CSV"), b"PK\x03\x04"),
            ImportFormat::Csv
        );
        assert_eq!(
            detect_import_format(None, b"PK\x03\x04rest"),
            ImportFormat::Xlsx
        );
        assert_eq!(
            detect_import_format(Some("notes.txt"), b"PK\x03\x04rest"),
            ImportFormat::Xlsx
        );
        assert_eq!(
            detect_import_format(None, b"a,b,c\n1,2,3\n"),
            ImportFormat::Csv
        );
    }

    #[test]
    fn sheet_name_from_filename_strips_paths_and_extensions_with_fallback() {
        assert_eq!(sheet_name_from_filename(None), "Sheet1");
        assert_eq!(sheet_name_from_filename(Some("")), "Sheet1");
        assert_eq!(sheet_name_from_filename(Some("   ")), "Sheet1");
        assert_eq!(sheet_name_from_filename(Some(".csv")), "Sheet1");
        assert_eq!(sheet_name_from_filename(Some("/tmp/.xlsx")), "Sheet1");
        assert_eq!(sheet_name_from_filename(Some("dir/sales.csv")), "sales");
        assert_eq!(
            sheet_name_from_filename(Some(r"C:\Users\demo\Q1 Report.xlsx")),
            "Q1 Report"
        );
        assert_eq!(sheet_name_from_filename(Some("plain")), "plain");
    }

    #[test]
    fn import_csv_bytes_parses_bom_quotes_ragged_types_and_formulas() {
        let mut raw = vec![0xEF, 0xBB, 0xBF];
        raw.extend_from_slice(
            b"Item,Amount,Active,Note\n\
              \"Rent, Apt\",1200,true,\"line1\nline2\"\n\
              Coffee,4.5,FALSE\n\
              Total,=SUM(B2:B3),,   \n\
              Edge,NaN,inf\n",
        );

        let wb = import_csv_bytes(&raw, "Budget").unwrap();
        assert_eq!(wb.sheet_order, vec![SheetId(1)]);
        let sheet = &wb.sheets[&SheetId(1)];
        assert_eq!(sheet.name, "Budget");
        assert_eq!(sheet.lattice, LatticeKind::Square);

        // BOM stripped from A1.
        assert_eq!(sheet.cells["A1"].value, CellValue::Text("Item".into()));
        // Quoted comma & newline preserved.
        assert_eq!(sheet.cells["A2"].value, CellValue::Text("Rent, Apt".into()));
        assert_eq!(sheet.cells["B2"].value, CellValue::Integer(1200));
        assert_eq!(sheet.cells["C2"].value, CellValue::Bool(true));
        assert_eq!(
            sheet.cells["D2"].value,
            CellValue::Text("line1\nline2".into())
        );
        // Ragged row (3 cols) + float + case-insensitive bool.
        assert_eq!(sheet.cells["B3"].value, CellValue::Number(4.5));
        assert_eq!(sheet.cells["C3"].value, CellValue::Bool(false));
        assert!(!sheet.cells.contains_key("D3"));
        // Formula stored unevaluated; blank/whitespace cells skipped.
        assert_eq!(sheet.cells["B4"].source.as_deref(), Some("=SUM(B2:B3)"));
        assert_eq!(sheet.cells["B4"].value, CellValue::Empty);
        assert!(!sheet.cells.contains_key("C4"));
        assert!(!sheet.cells.contains_key("D4"));
        // Non-finite float strings stay Text.
        assert_eq!(sheet.cells["B5"].value, CellValue::Text("NaN".into()));
        assert_eq!(sheet.cells["C5"].value, CellValue::Text("inf".into()));
    }

    #[test]
    fn import_csv_bytes_rejects_invalid_utf8() {
        let bad = b"ok,bad\n1,\xFF\xFE\n";
        let err = import_csv_bytes(bad, "Bad").unwrap_err();
        assert!(matches!(err, StoreError::Csv(_)));
    }
}
