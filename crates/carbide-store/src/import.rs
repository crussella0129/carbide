//! Byte-driven spreadsheet importers (`.csv` and `.xlsx`).
//!
//! All entrypoints take `&[u8]` so desktop native, headless CLI, and
//! WebAssembly targets share one implementation. Imported workbooks use
//! `LatticeKind::Square` sheets with canonical A1 addresses (`carbide-tess`),
//! preserving `=`-prefixed formulas so `WorkbookEngine` can build the DAG and
//! evaluate them immediately.

use std::io::Cursor;

use calamine::{CellErrorType, Data, Reader};
use carbide_core::{
    Cell, CellError, CellValue, EngineKind, Sheet, SheetExtent, SheetId, Workbook, WorkbookId,
    WorkbookMeta,
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
                    source: Some(field.trim_start().to_string()),
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

/// Map an Excel [`CellErrorType`] to Carbide's [`CellError`]. Standard
/// errors map directly; Excel-specific error codes (`#N/A`, `#NAME?`,
/// `#NULL!`, `#DATA!`) are preserved verbatim inside `CellError::Lang`.
fn map_excel_error(err: &CellErrorType) -> CellError {
    match err {
        CellErrorType::Div0 => CellError::DivZero,
        CellErrorType::Ref => CellError::Ref,
        CellErrorType::Num => CellError::Num,
        CellErrorType::Value => CellError::Value,
        CellErrorType::NA
        | CellErrorType::Name
        | CellErrorType::Null
        | CellErrorType::GettingData => CellError::Lang(err.to_string()),
    }
}

/// Convert a calamine [`Data`] value into a Carbide [`Cell`]. Returns `None`
/// for empty cells so imported sheets stay sparse.
fn cell_from_excel_data(data: &Data) -> Option<Cell> {
    match data {
        Data::Empty => None,
        Data::Int(i) => Some(Cell {
            source: Some(i.to_string()),
            engine: None,
            value: CellValue::Integer(*i),
        }),
        Data::Float(f) => {
            if f.is_finite() {
                Some(Cell {
                    source: Some(f.to_string()),
                    engine: None,
                    value: CellValue::Number(*f),
                })
            } else {
                Some(Cell {
                    source: None,
                    engine: None,
                    value: CellValue::Error(CellError::Num),
                })
            }
        }
        Data::Bool(b) => Some(Cell {
            source: Some(if *b { "TRUE" } else { "FALSE" }.to_string()),
            engine: None,
            value: CellValue::Bool(*b),
        }),
        Data::String(s) => {
            if s.is_empty() {
                return None;
            }
            // Literal Excel text starting with `=` (no `<f>` element) must
            // NOT carry `source: Some("=...")`, or `WorkbookEngine::rebuild_dag`
            // would misparse it as a live formula.
            let source = if s.trim_start().starts_with('=') {
                None
            } else {
                Some(s.clone())
            };
            Some(Cell {
                source,
                engine: None,
                value: CellValue::Text(s.clone()),
            })
        }
        Data::DateTime(dt) => {
            let serial = dt.as_f64();
            if serial.is_finite() {
                Some(Cell {
                    source: Some(serial.to_string()),
                    engine: None,
                    value: CellValue::Number(serial),
                })
            } else {
                Some(Cell {
                    source: None,
                    engine: None,
                    value: CellValue::Error(CellError::Num),
                })
            }
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => {
            if s.is_empty() {
                return None;
            }
            let source = if s.trim_start().starts_with('=') {
                None
            } else {
                Some(s.clone())
            };
            Some(Cell {
                source,
                engine: None,
                value: CellValue::Text(s.clone()),
            })
        }
        Data::Error(e) => Some(Cell {
            source: None,
            engine: None,
            value: CellValue::Error(map_excel_error(e)),
        }),
    }
}

fn checked_xlsx_coord(start: u32, rel: usize, axis: &str) -> Result<i32, StoreError> {
    let rel_u32 = u32::try_from(rel)
        .map_err(|_| StoreError::Format(format!("xlsx exceeds maximum {axis} index")))?;
    let sum = start
        .checked_add(rel_u32)
        .ok_or_else(|| StoreError::Format(format!("xlsx exceeds maximum {axis} index")))?;
    i32::try_from(sum).map_err(|_| StoreError::Format(format!("xlsx exceeds maximum {axis} index")))
}

/// Parse `.xlsx` workbook bytes via `calamine` into a multi-sheet
/// `LatticeKind::Square` [`Workbook`].
///
/// - Preserves all worksheets in workbook order.
/// - Merges `worksheet_range` and `worksheet_formula` independently using each
///   `Range`'s own `start()` origin so non-A1 sheets and formula-only cells
///   (without cached `<v>` elements) land at exact A1 coordinates.
/// - Prefixes non-empty formula strings with `=` while keeping any cached
///   value as the initial [`CellValue`] until `WorkbookEngine` evaluates them.
pub fn import_xlsx_bytes(bytes: &[u8]) -> Result<Workbook, StoreError> {
    let mut xlsx: calamine::Xlsx<_> = calamine::open_workbook_from_rs(Cursor::new(bytes))?;
    let sheet_names = xlsx.sheet_names();
    if sheet_names.is_empty() {
        return Err(StoreError::Format(
            "xlsx workbook contains no worksheets".into(),
        ));
    }

    let lattice = SquareLattice::default();
    let mut sheets = HashMap::new();
    let mut sheet_order = Vec::with_capacity(sheet_names.len());
    let title = sheet_names[0].clone();

    for (idx, name) in sheet_names.into_iter().enumerate() {
        let mut cells: HashMap<String, Cell> = HashMap::new();

        let val_range = xlsx.worksheet_range(&name)?;
        if let Some((start_r, start_c)) = val_range.start() {
            for (rel_r, rel_c, data) in val_range.used_cells() {
                let Some(cell) = cell_from_excel_data(data) else {
                    continue;
                };
                let row = checked_xlsx_coord(start_r, rel_r, "row")?;
                let col = checked_xlsx_coord(start_c, rel_c, "column")?;
                let addr = lattice.address(SquareCoord { col, row });
                cells.insert(addr, cell);
            }
        }

        let fml_range = xlsx.worksheet_formula(&name)?;
        if let Some((start_r, start_c)) = fml_range.start() {
            for (rel_r, rel_c, raw_formula) in fml_range.used_cells() {
                let trimmed = raw_formula.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let formula_src = if trimmed.starts_with('=') {
                    trimmed.to_string()
                } else {
                    format!("={trimmed}")
                };
                let row = checked_xlsx_coord(start_r, rel_r, "row")?;
                let col = checked_xlsx_coord(start_c, rel_c, "column")?;
                let addr = lattice.address(SquareCoord { col, row });
                cells
                    .entry(addr)
                    .and_modify(|cell| {
                        cell.source = Some(formula_src.clone());
                    })
                    .or_insert(Cell {
                        source: Some(formula_src),
                        engine: None,
                        value: CellValue::Empty,
                    });
            }
        }

        let sid = SheetId((idx + 1) as u32);
        sheet_order.push(sid);
        sheets.insert(
            sid,
            Sheet {
                id: sid,
                name,
                lattice: LatticeKind::Square,
                extent: SheetExtent::Unbounded,
                lattice_config: None,
                cells,
            },
        );
    }

    Ok(Workbook {
        id: WorkbookId(1),
        meta: WorkbookMeta {
            title,
            created_at: String::new(),
            format_version: FORMAT_VERSION,
        },
        default_engine: EngineKind::ExcelLite,
        sheet_order,
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

    /// Synthesize a valid minimal OOXML `.xlsx` archive in memory so tests
    /// exercise `calamine::Xlsx` without checking in binary fixtures.
    fn build_test_xlsx(sheets: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = Vec::new();
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let mut overrides = String::new();
        let mut wb_sheets = String::new();
        let mut wb_rels = String::new();
        for (i, (name, _)) in sheets.iter().enumerate() {
            let idx = i + 1;
            overrides.push_str(&format!(
                r#"<Override PartName="/xl/worksheets/sheet{idx}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
            ));
            wb_sheets.push_str(&format!(
                r#"<sheet name="{name}" sheetId="{idx}" r:id="rId{idx}"/>"#
            ));
            wb_rels.push_str(&format!(
                r#"<Relationship Id="rId{idx}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{idx}.xml"/>"#
            ));
        }
        let style_rid = sheets.len() + 1;
        wb_rels.push_str(&format!(
            r#"<Relationship Id="rId{style_rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
        ));

        zip.start_file("[Content_Types].xml", opts).unwrap();
        write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
  {overrides}
</Types>"#
        )
        .unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#
        )
        .unwrap();

        zip.start_file("xl/workbook.xml", opts).unwrap();
        write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>{wb_sheets}</sheets>
</workbook>"#
        )
        .unwrap();

        zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
        write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {wb_rels}
</Relationships>"#
        )
        .unwrap();

        // Style index 0 = general, style index 1 = built-in date format (numFmtId=14).
        zip.start_file("xl/styles.xml", opts).unwrap();
        write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="2">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
    <xf numFmtId="14" fontId="0" fillId="0" borderId="0" applyNumberFormat="1"/>
  </cellXfs>
</styleSheet>"#
        )
        .unwrap();

        for (i, (_, sheet_data)) in sheets.iter().enumerate() {
            let idx = i + 1;
            zip.start_file(format!("xl/worksheets/sheet{idx}.xml"), opts)
                .unwrap();
            write!(
                zip,
                r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>{sheet_data}</sheetData>
</worksheet>"#
            )
            .unwrap();
        }

        zip.finish().unwrap();
        buf
    }

    #[test]
    fn import_xlsx_bytes_handles_multisheet_offsets_formulas_dates_and_errors() {
        let sheet1 = r#"
<row r="2">
  <c r="B2"><v>10</v></c>
  <c r="C2"><v>2.5</v></c>
  <c r="D2" t="b"><v>1</v></c>
  <c r="E2" t="inlineStr"><is><t>Hello</t></is></c>
  <c r="F2" t="inlineStr"><is><t>=literal_text</t></is></c>
</row>
<row r="3">
  <c r="B3"><f>SUM(B2:C2)</f><v>12.5</v></c>
  <c r="C3"><f>B2*3</f></c>
  <c r="D3" s="1"><v>45000</v></c>
  <c r="E3" t="d"><v>2026-11-27</v></c>
  <c r="F3" t="e"><v>#DIV/0!</v></c>
</row>"#;
        let sheet2 = ""; // Empty sheet (`Range::start() == None`)
        let sheet3 = r#"<row r="1"><c r="A1"><v>99</v></c></row>"#;

        let bytes = build_test_xlsx(&[
            ("Sales", sheet1),
            ("EmptySheet", sheet2),
            ("Summary", sheet3),
        ]);
        let wb = import_xlsx_bytes(&bytes).unwrap();
        assert_eq!(wb.sheet_order.len(), 3);

        let s1 = &wb.sheets[&wb.sheet_order[0]];
        assert_eq!(s1.name, "Sales");
        assert_eq!(s1.lattice, LatticeKind::Square);
        // Non-A1 offset preserved: row 2 col B -> B2.
        assert!(!s1.cells.contains_key("A1"));
        assert_eq!(s1.cells["B2"].value, CellValue::Number(10.0));
        assert_eq!(s1.cells["C2"].value, CellValue::Number(2.5));
        assert_eq!(s1.cells["D2"].value, CellValue::Bool(true));
        assert_eq!(s1.cells["E2"].value, CellValue::Text("Hello".into()));
        // Literal string starting with '=' must NOT set source="=..."
        assert_eq!(
            s1.cells["F2"].value,
            CellValue::Text("=literal_text".into())
        );
        assert_eq!(s1.cells["F2"].source, None);
        // Formula with cached value preserves both.
        assert_eq!(s1.cells["B3"].source.as_deref(), Some("=SUM(B2:C2)"));
        assert_eq!(s1.cells["B3"].value, CellValue::Number(12.5));
        // Formula-only cell (no `<v>`) is merged independently from worksheet_formula.
        assert_eq!(s1.cells["C3"].source.as_deref(), Some("=B2*3"));
        assert_eq!(s1.cells["C3"].value, CellValue::Empty);
        // DateTime (serial) & DateTimeIso.
        assert_eq!(s1.cells["D3"].value, CellValue::Number(45000.0));
        assert_eq!(s1.cells["E3"].value, CellValue::Text("2026-11-27".into()));
        // Excel error cell.
        assert_eq!(s1.cells["F3"].value, CellValue::Error(CellError::DivZero));

        // Empty sheet succeeds with 0 cells.
        let s2 = &wb.sheets[&wb.sheet_order[1]];
        assert_eq!(s2.name, "EmptySheet");
        assert!(s2.cells.is_empty());

        // Third sheet preserved in order.
        let s3 = &wb.sheets[&wb.sheet_order[2]];
        assert_eq!(s3.name, "Summary");
        assert_eq!(s3.cells["A1"].value, CellValue::Number(99.0));
    }

    #[test]
    fn excel_error_and_non_finite_float_mapping_covers_all_variants() {
        assert_eq!(map_excel_error(&CellErrorType::Div0), CellError::DivZero);
        assert_eq!(map_excel_error(&CellErrorType::Ref), CellError::Ref);
        assert_eq!(map_excel_error(&CellErrorType::Num), CellError::Num);
        assert_eq!(map_excel_error(&CellErrorType::Value), CellError::Value);
        assert_eq!(
            map_excel_error(&CellErrorType::NA),
            CellError::Lang("#N/A".into())
        );
        assert_eq!(
            map_excel_error(&CellErrorType::Name),
            CellError::Lang("#NAME?".into())
        );
        assert_eq!(
            map_excel_error(&CellErrorType::Null),
            CellError::Lang("#NULL!".into())
        );
        assert_eq!(
            map_excel_error(&CellErrorType::GettingData),
            CellError::Lang("#DATA!".into())
        );

        let nan_cell = cell_from_excel_data(&Data::Float(f64::NAN)).unwrap();
        assert_eq!(nan_cell.value, CellValue::Error(CellError::Num));
        assert_eq!(nan_cell.source, None);
        let inf_cell = cell_from_excel_data(&Data::Float(f64::INFINITY)).unwrap();
        assert_eq!(inf_cell.value, CellValue::Error(CellError::Num));

        let int_cell = cell_from_excel_data(&Data::Int(42)).unwrap();
        assert_eq!(int_cell.value, CellValue::Integer(42));
        assert_eq!(int_cell.source.as_deref(), Some("42"));

        let dur_cell = cell_from_excel_data(&Data::DurationIso("PT1H30M".into())).unwrap();
        assert_eq!(dur_cell.value, CellValue::Text("PT1H30M".into()));
        assert_eq!(dur_cell.source.as_deref(), Some("PT1H30M"));

        // Leading whitespace before `=` in CSV is trimmed so ExcelLite parses it.
        let wb = import_csv_bytes(b"10, =A1*2\n", "Trim").unwrap();
        let sheet = &wb.sheets[&wb.sheet_order[0]];
        assert_eq!(sheet.cells["B1"].source.as_deref(), Some("=A1*2"));
    }

    #[test]
    fn import_xlsx_bytes_rejects_corrupt_and_sheetless_inputs() {
        assert!(import_xlsx_bytes(b"not a zip archive").is_err());
        let empty_wb = build_test_xlsx(&[]);
        assert!(matches!(
            import_xlsx_bytes(&empty_wb),
            Err(StoreError::Format(_))
        ));
    }
}
