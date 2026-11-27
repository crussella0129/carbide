//! Property-based tests for `.crbd` persistence — v14.
//!
//! A spreadsheet that loses data when saved and reopened is worse than
//! useless, so the `save` → `open` round-trip is a correctness property
//! that must hold for *every* workbook, not just the one shape the
//! existing `save_and_open_preserves_dependencies` unit test covers.
//!
//! These tests generate random workbooks, drive them through
//! `WorkbookEngine::save` (→ `carbide_store`) and `WorkbookEngine::open`
//! (← `carbide_store`), and assert:
//!
//!  * every cell's source text and computed value survive the round-trip;
//!  * the reopened workbook's DAG is functional — `open` rebuilds the
//!    dependency graph by re-parsing sources, so a post-open edit must
//!    still propagate to dependents;
//!  * a multi-sheet workbook mixing a square and a hex lattice round-trips
//!    intact, lattice kinds included.
//!
//! Determinism is fixed-seed so a CI failure always reproduces.

use std::path::PathBuf;

use carbide_core::{CellValue, SheetId};
use carbide_formula::WorkbookEngine;
use carbide_tess::LatticeKind;

/// Fixed seed — a generative CI gate must reproduce byte-for-byte.
const SEED: u64 = 0x7E5C_E11A_7E5D_0001;
/// How many random workbooks to generate per property.
const WORKBOOKS: usize = 60;

/// xorshift64 — a tiny, dependency-free, deterministic PRNG.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n)) as u32
    }
}

/// A small positive integer literal, `1..=9`.
fn lit(rng: &mut Rng) -> u32 {
    1 + rng.below(9)
}

/// A random workbook as `(address, source)` pairs. `A1` is a literal and
/// `A2` always references `A1` (so there is a guaranteed dependency to
/// exercise after reopening); later cells are literals or arithmetic over
/// strictly-earlier cells — acyclic, and free of array literals so no cell
/// spills.
fn gen_workbook(rng: &mut Rng) -> Vec<(String, String)> {
    let n = 6 + rng.below(15) as usize; // 6..=20 cells
    let mut cells = vec![
        ("A1".to_string(), format!("={}", lit(rng))),
        ("A2".to_string(), format!("=A1 + {}", lit(rng))),
    ];
    for i in 2..n {
        let addr = format!("A{}", i + 1);
        let src = if rng.below(3) == 0 {
            format!("={}", lit(rng))
        } else {
            let a = rng.below(i as u32) + 1;
            let b = rng.below(i as u32) + 1;
            let o = ["+", "-", "*"][rng.below(3) as usize];
            format!("=A{a} {o} A{b}")
        };
        cells.push((addr, src));
    }
    cells
}

/// Build a fresh single-sheet (square) engine with `cells` applied.
fn build(cells: &[(String, String)]) -> (WorkbookEngine, SheetId) {
    let mut eng = WorkbookEngine::new();
    eng.new_workbook();
    let sid = eng.add_sheet("Sheet1", LatticeKind::Square);
    for (addr, src) in cells {
        eng.set_cell(sid, addr, Some(src)).expect("set_cell");
    }
    (eng, sid)
}

/// A unique-per-call temp `.crbd` path.
fn temp_tscl(tag: &str, i: usize) -> PathBuf {
    std::env::temp_dir().join(format!(
        "carbide-persist-{tag}-{}-{i}.crbd",
        std::process::id()
    ))
}

#[test]
fn save_open_round_trips_values_and_sources() {
    let mut rng = Rng::new(SEED);
    for w in 0..WORKBOOKS {
        let cells = gen_workbook(&mut rng);
        let (eng, sid) = build(&cells);

        // Capture every cell's source + value before the round-trip.
        let before: Vec<(String, Option<String>, CellValue)> = cells
            .iter()
            .map(|(addr, _)| {
                let snap = eng.get_cell(sid, addr).expect("cell exists");
                (addr.clone(), snap.source, snap.value)
            })
            .collect();

        let path = temp_tscl("roundtrip", w);
        eng.save(&path).expect("save");
        let mut reopened = WorkbookEngine::new();
        reopened.open(&path).expect("open");
        std::fs::remove_file(&path).ok();

        for (addr, src, val) in &before {
            let snap = reopened
                .get_cell(sid, addr)
                .unwrap_or_else(|| panic!("workbook {w}: cell {addr} missing after reopen"));
            assert_eq!(
                &snap.source, src,
                "workbook {w}: source of {addr} changed across save/open",
            );
            assert_eq!(
                &snap.value, val,
                "workbook {w}: value of {addr} changed across save/open",
            );
        }
    }
}

#[test]
fn reopened_workbook_has_a_working_dag() {
    let mut rng = Rng::new(SEED);
    for w in 0..WORKBOOKS {
        let cells = gen_workbook(&mut rng);
        let (eng, sid) = build(&cells);

        let path = temp_tscl("dag", w);
        eng.save(&path).expect("save");
        let mut reopened = WorkbookEngine::new();
        reopened.open(&path).expect("open");
        std::fs::remove_file(&path).ok();

        // `A2` always reads `A1`. `open` rebuilds the DAG from sources, so
        // editing `A1` must recompute `A2` in the reopened workbook.
        let a2_before = reopened.get_cell(sid, "A2").expect("A2 exists").value;
        let changed = reopened
            .set_cell(sid, "A1", Some("=999"))
            .expect("set_cell after reopen");
        assert!(
            changed.iter().any(|c| c.address == "A2"),
            "workbook {w}: editing A1 did not recompute A2 — DAG not rebuilt on open",
        );
        let a2_after = reopened.get_cell(sid, "A2").expect("A2 exists").value;
        assert_ne!(
            a2_before, a2_after,
            "workbook {w}: A2 value did not change after editing A1 post-reopen",
        );
    }
}

#[test]
fn multi_sheet_and_hex_lattice_round_trip() {
    // A workbook with two sheets on two different lattices.
    let mut eng = WorkbookEngine::new();
    eng.new_workbook();
    let sq = eng.add_sheet("Numbers", LatticeKind::Square);
    let hx = eng.add_sheet("HexGrid", LatticeKind::HexPointy);

    eng.set_cell(sq, "A1", Some("=10")).unwrap();
    eng.set_cell(sq, "A2", Some("=A1 * 3")).unwrap();
    eng.set_cell(hx, "H(0,0)", Some("=7")).unwrap();
    eng.set_cell(hx, "H(1,-1)", Some("=H(0,0) + 5")).unwrap();

    let path = temp_tscl("multisheet", 0);
    eng.save(&path).unwrap();
    let mut reopened = WorkbookEngine::new();
    reopened.open(&path).unwrap();
    std::fs::remove_file(&path).ok();

    // Both sheets and both lattices survive: the hex cells are only
    // addressable at all if the hex lattice round-tripped.
    assert_eq!(
        reopened.get_cell(sq, "A2").expect("square A2").value,
        CellValue::Number(30.0),
    );
    assert_eq!(
        reopened.get_cell(hx, "H(1,-1)").expect("hex H(1,-1)").value,
        CellValue::Number(12.0),
    );

    // The reopened hex sheet's DAG is functional too.
    reopened.set_cell(hx, "H(0,0)", Some("=100")).unwrap();
    assert_eq!(
        reopened.get_cell(hx, "H(1,-1)").expect("hex H(1,-1)").value,
        CellValue::Number(105.0),
        "editing H(0,0) did not recompute H(1,-1) after reopen",
    );
}

/// Minimal dependency-free ZIP (STORE method) builder for synthesizing valid
/// `.xlsx` archives in integration tests without adding `zip` to dev-deps.
fn build_stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
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
    for &(name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let len = data.len() as u32;
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u16;

        // Local file header
        out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // STORE
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        // Central directory header
        central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes()); // flags
        central.extend_from_slice(&0u16.to_le_bytes()); // STORE
        central.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central.extend_from_slice(&0u16.to_le_bytes()); // mod date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra len
        central.extend_from_slice(&0u16.to_le_bytes()); // comment len
        central.extend_from_slice(&0u16.to_le_bytes()); // disk start
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    let count = files.len() as u16;
    out.extend_from_slice(&central);
    // End of central directory
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn minimal_xlsx(sheets: &[(&str, &str)]) -> Vec<u8> {
    let mut overrides = String::new();
    let mut wb_sheets = String::new();
    let mut wb_rels = String::new();
    let mut sheet_xmls: Vec<(String, String)> = Vec::new();
    for (i, (name, body)) in sheets.iter().enumerate() {
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
        sheet_xmls.push((
            format!("xl/worksheets/sheet{idx}.xml"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{body}</sheetData></worksheet>"#
            ),
        ));
    }
    let ct = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>{overrides}</Types>"#
    );
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let wb = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{wb_sheets}</sheets></workbook>"#
    );
    let wb_r = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{wb_rels}</Relationships>"#
    );

    let mut entries: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", ct.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("xl/workbook.xml", wb.as_bytes()),
        ("xl/_rels/workbook.xml.rels", wb_r.as_bytes()),
    ];
    for (path, xml) in &sheet_xmls {
        entries.push((path.as_str(), xml.as_bytes()));
    }
    build_stored_zip(&entries)
}

#[test]
fn import_csv_bytes_evaluates_formulas_recomputes_and_roundtrips_crbd() {
    let csv = b"10,20,=A1+B1\n=C1*2\n";
    let mut eng = WorkbookEngine::new();
    let order = eng.import_csv_bytes(csv, "Sales").unwrap();
    let sid = order[0];

    assert_eq!(
        eng.get_cell(sid, "C1").unwrap().value,
        CellValue::Number(30.0)
    );
    assert_eq!(
        eng.get_cell(sid, "A2").unwrap().value,
        CellValue::Number(60.0)
    );

    // Recompute chain works live after import.
    eng.set_cell(sid, "A1", Some("100")).unwrap();
    assert_eq!(
        eng.get_cell(sid, "C1").unwrap().value,
        CellValue::Number(120.0)
    );
    assert_eq!(
        eng.get_cell(sid, "A2").unwrap().value,
        CellValue::Number(240.0)
    );

    // Round-trip through `.crbd` v2 bytes.
    let bytes = eng.save_bytes(&carbide_store::UiState::default()).unwrap();
    let mut reopened = WorkbookEngine::new();
    reopened.open_bytes(&bytes).unwrap();
    assert_eq!(
        reopened.get_cell(sid, "A2").unwrap().value,
        CellValue::Number(240.0)
    );
}

#[test]
fn import_csv_bytes_evaluates_volatile_dependents_and_marks_cycles() {
    // B1 is volatile (`INDIRECT("A1")`), C1 depends on B1; D1/E1 form a cycle.
    let csv = b"=10+5,=INDIRECT(\"A1\"),=B1*2,=E1,=D1\n";
    let mut eng = WorkbookEngine::new();
    let sid = eng.import_csv_bytes(csv, "VolCycle").unwrap()[0];

    assert_eq!(
        eng.get_cell(sid, "A1").unwrap().value,
        CellValue::Number(15.0)
    );
    assert_eq!(
        eng.get_cell(sid, "B1").unwrap().value,
        CellValue::Number(15.0)
    );
    assert_eq!(
        eng.get_cell(sid, "C1").unwrap().value,
        CellValue::Number(30.0)
    );
    assert_eq!(
        eng.get_cell(sid, "D1").unwrap().value,
        CellValue::Error(carbide_core::CellError::Cycle)
    );
    assert_eq!(
        eng.get_cell(sid, "E1").unwrap().value,
        CellValue::Error(carbide_core::CellError::Cycle)
    );

    // Editing A1 must still recompute volatile B1 and its dependent C1.
    eng.set_cell(sid, "A1", Some("50")).unwrap();
    assert_eq!(
        eng.get_cell(sid, "B1").unwrap().value,
        CellValue::Number(50.0)
    );
}

#[test]
fn import_xlsx_bytes_evaluates_multisheet_and_preserves_cached_unsupported_formula() {
    let s1 = r#"<row r="1"><c r="A1"><v>7</v></c><c r="B1"><v>8</v></c><c r="C1"><f>SUM(A1:B1)</f></c><c r="D1"><f>UNSUPPORTED_SYNTAX((</f><v>123</v></c></row>"#;
    let s2 = r#"<row r="1"><c r="A1"><v>4</v></c><c r="B1"><f>A1*5</f></c></row>"#;
    let bytes = minimal_xlsx(&[("Main", s1), ("Second", s2)]);

    let mut eng = WorkbookEngine::new();
    let order = eng.import_xlsx_bytes(&bytes).unwrap();
    assert_eq!(order.len(), 2);

    assert_eq!(
        eng.get_cell(order[0], "C1").unwrap().value,
        CellValue::Number(15.0)
    );
    // Unparseable Excel formula keeps cached value without aborting import.
    assert_eq!(
        eng.get_cell(order[0], "D1").unwrap().value,
        CellValue::Number(123.0)
    );
    assert_eq!(
        eng.get_cell(order[1], "B1").unwrap().value,
        CellValue::Number(20.0)
    );
}

#[test]
fn replace_sheet_cells_evaluates_target_and_preserves_sibling_sheets() {
    let mut eng = WorkbookEngine::new();
    eng.new_workbook();
    let sq = eng.add_sheet("Budget", LatticeKind::Square);
    let hx = eng.add_sheet("Hex Game", LatticeKind::HexPointy);

    eng.set_cell(sq, "A1", Some("=999")).unwrap();
    eng.set_cell(hx, "H(2,2)", Some("=RANDBETWEEN(1, 1000000)"))
        .unwrap();
    eng.set_cell(hx, "H(3,2)", Some("=H(2,2)")).unwrap();
    let hex_roll_before = eng.get_cell(hx, "H(2,2)").unwrap().value;

    let imported = carbide_store::import::import_csv_bytes(b"3,4,=A1+B1\n", "Imported").unwrap();
    let imp_sheet = imported.sheets.into_values().next().unwrap();
    eng.replace_sheet_cells(sq, Some(imp_sheet.name), imp_sheet.cells)
        .unwrap();

    assert_eq!(eng.workbook.sheets[&sq].name, "Imported");
    assert_eq!(
        eng.get_cell(sq, "C1").unwrap().value,
        CellValue::Number(7.0)
    );
    // Sibling Hex sheet's RANDBETWEEN cell must NOT be re-evaluated.
    assert_eq!(eng.get_cell(hx, "H(2,2)").unwrap().value, hex_roll_before);
    assert_eq!(eng.get_cell(hx, "H(3,2)").unwrap().value, hex_roll_before);
}

#[cfg(feature = "native")]
#[test]
fn replace_sheet_cells_clears_stale_native_programs() {
    let mut eng = WorkbookEngine::new();
    eng.new_workbook();
    let sq = eng.add_sheet("Sheet1", LatticeKind::Square);
    eng.set_cell(sq, "A1", Some("=10")).unwrap();
    eng.set_cell(sq, "A2", Some("=A1 + 1")).unwrap();
    eng.promote_native(sq, "A2").unwrap();
    assert_eq!(
        eng.get_cell(sq, "A2").unwrap().value,
        CellValue::Number(11.0)
    );

    // Replace A2 with a completely different formula; stale cdylib must not run.
    let imported = carbide_store::import::import_csv_bytes(b"10\n=A1*100\n", "Replaced").unwrap();
    let imp_sheet = imported.sheets.into_values().next().unwrap();
    eng.replace_sheet_cells(sq, Some(imp_sheet.name), imp_sheet.cells)
        .unwrap();
    assert_eq!(
        eng.get_cell(sq, "A2").unwrap().value,
        CellValue::Number(1000.0)
    );
}
