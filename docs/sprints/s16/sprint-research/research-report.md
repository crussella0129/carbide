# Sprint 16 Research Report — CSV and `.xlsx` Spreadsheet Import

## Intents Reviewed
- [INT-0005 — Post-rename product feature roadmap](../../../intents/INT-0005-post-rename-product-roadmap.md) (umbrella roadmap; item 1 split into `INT-0006`)
- [INT-0006 — CSV and `.xlsx` spreadsheet import](../../../intents/INT-0006-csv-and-xlsx-import.md) (state: `proposed`)

## Summary
Researched the next planned post-rename feature per ADR-015 (`docs/history/decisions-legacy.md:402-403`): importing `.csv` and `.xlsx` files into Carbide using `csv` and `calamine`. Verified crate compatibility with our Sprint 15 MSRVs (`1.88` root / `1.95` UI) and `wasm32-unknown-unknown`, mapped the exact boundaries across `carbide-store`, `carbide-formula`, and `apps/carbide-ui`, and identified an existing UI gap where `ribbon.rs:698-705` still disabled `File -> Open…` and `File -> Save` despite working handlers in `commands.rs`.

## Findings

### 1. Crate Selection, MSRV & `wasm32` Compatibility
- **`csv = "1.4"`** (`1.4.0`, MSRV `1.73`, pure Rust, `wasm32`-compatible): supports flexible row lengths (`flexible(true)`), `has_headers(false)` (since row 1 of a spreadsheet is cell row 1 `A1`, not discarded!), and reading directly from `&[u8]`.
- **`calamine = "0.38"`** (`0.38.0`, MSRV `1.88`, `default = []`, pure Rust, `wasm32`-compatible): matches root MSRV `1.88` exactly (zero MSRV bump needed!). Exposes `Xlsx<Cursor<&[u8]>>` via `calamine::open_workbook_from_rs(Cursor::new(bytes))`, `sheet_names()`, `worksheet_range(name) -> Range<Data>`, and `worksheet_formula(name) -> Range<String>`.
- Note: `calamine 0.38` pulls `zip 9.0` internally (`default-features = false, features = ["deflate"]`), which coexists cleanly in Cargo alongside `carbide-store`'s `zip 2` without altering `.crbd` v2 serialization.
- `carbide-tess` is currently in `[dev-dependencies]` of `crates/carbide-store/Cargo.toml` (already a transitive dependency via `carbide-core`); promoting `carbide-tess = { workspace = true }` to `[dependencies]` lets `carbide-store::import` reuse `SquareLattice::default().address(SquareCoord { col, row })` with zero duplicate A1 formatting code.

### 2. Import Semantics (`carbide-store::import`)
- **CSV (`import_csv_bytes(bytes: &[u8], sheet_name: &str) -> Result<Workbook, StoreError>`):**
  - Strip optional UTF-8 BOM (`\u{FEFF}`) before parsing so Excel-exported UTF-8 CSVs don't corrupt `A1`.
  - Use `csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(bytes)`.
  - For each `(row_idx, record)` and `(col_idx, raw_field)`:
    - Skip empty/blank fields (sparse sheet invariant).
    - If `raw_field.trim_start().starts_with('=')`: store `Cell { source: Some(raw_field.to_string()), engine: None, value: CellValue::Empty }` so `WorkbookEngine` parses and evaluates it.
    - Otherwise infer typed literal matching Carbide's `parse_literal`: `i64` → `CellValue::Integer`, finite `f64` → `CellValue::Number`, `TRUE`/`FALSE` (case-insensitive) → `CellValue::Bool`, else `CellValue::Text`, preserving `source: Some(raw_field.to_string())`.
- **XLSX (`import_xlsx_bytes(bytes: &[u8]) -> Result<Workbook, StoreError>`):**
  - Open via `calamine::open_workbook_from_rs::<calamine::Xlsx<_>, _>(Cursor::new(bytes))`.
  - Iterate `sheet_names()` in workbook order; create one `LatticeKind::Square` `Sheet` per worksheet (with sequential `SheetId(1..=N)`). If a workbook has zero sheets, return `StoreError::Format(...)`.
  - For each worksheet:
    - Read values via `worksheet_range(&name)` and formulas via `worksheet_formula(&name)` (if `worksheet_formula` returns a `Range<String>`, map `(abs_row, abs_col)` using `range.start()` offset!).
    - **Formula precedence:** If a cell has a non-empty formula string `f`, store `source = Some(if f.starts_with('=') { f } else { format!("={f}") })` and keep the cached `Data` value (if any) as initial `CellValue` until `WorkbookEngine` evaluates it.
    - **Typed `Data` mapping:** `Data::Int(i)` → `CellValue::Integer(i)` + `source: Some(i.to_string())`; `Data::Float(f)` → `CellValue::Number(f)` (if `f.fract() == 0.0 && f.abs() < 1e15` keep clean formatting, or `f.to_string()`); `Data::Bool(b)` → `CellValue::Bool(b)` + `Some("TRUE"/"FALSE")`; `Data::String(s)` → skip if empty, else `CellValue::Text(s.clone())` + `Some(s)`; `Data::DateTime(dt)` → `CellValue::Number(dt.as_f64())` + `Some(dt.as_f64().to_string())`; `Data::DateTimeIso(s)` / `Data::DurationIso(s)` → `CellValue::Text(s)` + `Some(s)`; `Data::Error(e)` → mapped `CellValue::Error(...)`; `Data::Empty` → skip.
- **Format detection helper (`detect_import_format(filename: Option<&str>, bytes: &[u8]) -> ImportFormat`):**
  - Inspect file extension (`.xlsx` vs `.csv`, case-insensitive) and fall back to zip magic bytes (`bytes.starts_with(b"PK\x03\x04")`) for `.xlsx`, else `Csv`. Enables clean single-picker UX on both native and `wasm32` (where `rfd::FileHandle::file_name()` provides the file name).
- **Test fixtures without binary blobs:** In `carbide-store` unit tests, we can synthesize valid minimal `.xlsx` zip archives in pure Rust using `zip::ZipWriter` (`[Content_Types].xml`, `_rels/.rels`, `xl/workbook.xml`, `xl/_rels/workbook.xml.rels`, `xl/worksheets/sheet1.xml`, `xl/worksheets/sheet2.xml`) — proving real multi-sheet `.xlsx` parsing with ints, floats, booleans, inline strings, and formulas (`<f>SUM(A1:A2)</f>`) with zero checked-in binary fixtures!

### 3. Engine Integration (`crates/carbide-formula/src/engine.rs`)
- Currently `rebuild_dag()` (`engine.rs:154-234`) rebuilds DAG edges and `self.compiled`, but neither populates `self.volatile` nor evaluates formula cells (which `.crbd` didn't need because `.crbd` persisted cached values, whereas imported CSV/XLSX formula cells must be evaluated!).
- Adding:
  - `rebuild_dag()` updated to also record `self.volatile` (matching `set_cell`'s `OFFSET(` / `INDIRECT(` detection).
  - `evaluate_all_formulas(&mut self)` that computes `self.dag.topo_order(all_compiled)` + remaining nodes + volatile second pass and calls `self.recompute(&cref)` for every formula cell.
  - `import_csv_bytes(&mut self, bytes: &[u8], sheet_name: &str) -> Result<Vec<SheetId>, SetCellError>` and `import_xlsx_bytes(&mut self, bytes: &[u8]) -> Result<Vec<SheetId>, SetCellError>` (plus native path wrappers `import_csv` / `import_xlsx`).
  - `replace_sheet_cells(&mut self, sheet: SheetId, name: Option<String>, cells: HashMap<String, Cell>) -> Result<(), SetCellError>`: replaces the cells (and optionally renames) a target sheet in-place while preserving all sibling sheets (Hex/Triangle/Voronoi), clears & rebuilds the DAG + volatile set, and evaluates all formulas cleanly!

### 4. UI Integration (`apps/carbide-ui`)
- **File Menu Parity (`ribbon.rs:698-711`):** Enable `File -> Open…` (`RibbonAction::Open`), `File -> Save` (`RibbonAction::Save`), and add `File -> Import CSV / Excel…` (`RibbonAction::Import`), removing the stale disabled `"(Save/Open need engine support)"` label!
- **Async-compatible Import Slot (`app/mod.rs`, `app/io.rs`):** Mirror `pending_open_bytes` with `pending_import: Arc<Mutex<Option<(Option<String>, Vec<u8>)>>>` + `import_error: Option<String>` (shown as a dismissible status/banner if a file fails to parse, rather than silently failing to stderr).
- **`handle_import` & `drain_pending_import` (`app/io.rs`):**
  - Native (`rfd::FileDialog`) and `wasm32` (`rfd::AsyncFileDialog`) pick `.csv` / `.xlsx` (`add_filter("Spreadsheets (CSV, Excel)", &["csv", "xlsx"])`).
  - On arrival, detect format via `detect_import_format`, parse via `carbide_store::import::{import_csv_bytes, import_xlsx_bytes}`, apply the first imported sheet into `self.square.sheet_id` via `self.engine.replace_sheet_cells(...)` (keeping Hex/Triangle/Voronoi tabs 100% live and valid!), switch `self.active = ActiveSheet::Square`, clear stale square formats/notes/widgets/history and reset square cursor to `(0, 0)`, and update the Square tab label in `app/mod.rs:562` dynamically from `self.engine.workbook.sheets.get(&self.square.sheet_id)`!
  - Note on `rfd::FileHandle::file_name()` on `wasm32`: returns `String`, passed as `Some(handle.file_name())`; native passes `path.file_name().and_then(|s| s.to_str()).map(str::to_string)`.

## Gaps & Risks
- **Excel formula syntax differences:** Excel workbooks may contain formulas unsupported by Carbide's `ExcelLite` parser (e.g. `XLOOKUP` or cross-sheet `Sheet2!A1`). `rebuild_dag` and `evaluate_all_formulas` must degrade gracefully on unparseable or unsupported formulas (preserving cached Excel value or recording a cell error rather than aborting the entire import!).
- **Large CSV/XLSX vs `COLS`/`ROWS` UI grid (`52 × 200`):** The square UI currently renders `COLS = 52`, `ROWS = 200`, while the engine's square sheet is unbounded (`SheetExtent::Unbounded`). Imported cells beyond `52 × 200` remain intact in the engine and `.crbd` round-trip without panicking in the renderer.
- **Module size & lint hygiene:** Every file under `apps/carbide-ui/src/app/` must stay ≤ 1,000 lines, zero `#![allow(...)]`/`#[allow(...)]` attributes, and both native AND `wasm32-unknown-unknown` must pass `clippy --all-targets -- -D warnings`.

## Sources
1. `docs/intents/INT-0005-post-rename-product-roadmap.md` & `docs/history/decisions-legacy.md` (ADR-015 deferred CSV/`.xlsx` item).
2. `crates/carbide-store/Cargo.toml` & `crates/carbide-store/src/lib.rs` — `.crbd` v2 persistence layer and `StoreError`.
3. `crates/carbide-formula/src/engine.rs` — `WorkbookEngine::open_bytes`, `rebuild_dag`, `add_sheet`, `set_cell`.
4. `apps/carbide-ui/src/app/io.rs`, `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/ribbon.rs` — File menu, `rfd` native/wasm32 dialogs, and `pending_open_bytes` pattern.
5. `csv 1.4.0` & `calamine 0.38.0` crate metadata (`Cargo.toml`, `datatype.rs`, `lib.rs`).
