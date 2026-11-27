# Sprint 16 Build Plan

## Intents
- [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md) — state: `planned`; acceptance criteria covered: AC-1 (byte-driven `carbide-store::import` CSV + `.xlsx` parsers via `csv 1.4` and `calamine 0.38` without changing `.crbd` `FORMAT_VERSION = 2`), AC-2 (`WorkbookEngine` `import_csv_bytes`/`import_xlsx_bytes`/`import_csv`/`import_xlsx`/`replace_sheet_cells` with DAG rebuild, volatile tracking, cycle marking, native cache invalidation, and sheet/workbook-scoped topological formula evaluation), AC-3 (`apps/carbide-ui` File menu `Open…`/`Save`/`Import CSV / Excel…`, native + `wasm32` `.csv`/`.xlsx` picker with repaint & autosave wiring, full stale square-state reset, non-square tab preservation, dynamic square tab name, multi-sheet notice, and non-fatal import error reporting).

## Schema Tree
- Sprint 16: CSV and `.xlsx` spreadsheet import (`INT-0006`)
  - Store layer: CSV parsing, format detection & MSRV 1.89 alignment
    - T-001: Add `csv 1.4` + `calamine 0.38` to `carbide-store`, align root MSRV to `"1.89"` (`zip 9.8.2`), and implement `import_csv_bytes`, `detect_import_format`, and `sheet_name_from_filename` with unit tests
  - Store layer: `.xlsx` parsing via `calamine`
    - T-002: Implement `carbide-store::import::import_xlsx_bytes` (independent `used_cells` + `start()` merge for values & formula-only cells, finite-float guard, full `CellErrorType` mapping, literal `=…` string safety) with pure-Rust `.xlsx` zip unit tests
  - Engine layer: DAG rebuild, cycle/volatile/native safety & sheet-scoped evaluation
    - T-003: Add `WorkbookEngine` `import_csv_bytes`, `import_xlsx_bytes`, `import_csv`, `import_xlsx`, `replace_sheet_cells`, and `evaluate_sheet_formulas`/`evaluate_all_formulas` (clearing `self.native`, marking cycles `CellError::Cycle`, recomputing volatile `dirty_closure`, preserving sibling sheets) with integration tests
  - UI layer: File menu parity & native/wasm32 import flow
    - T-004: Enable `File -> Open…`/`Save` and add `File -> Import CSV / Excel…` in `ribbon.rs`, wire `handle_import`/`drain_pending_import` (with `ctx.request_repaint()`, `mark_dirty()`, full stale-state reset, multi-sheet notice, non-fatal error display, and headless unit tests) in `apps/carbide-ui`

## Architecture & Invariants (Incorporating Plan Critic C-001..C-005)
- **MSRV & `.crbd` v2 preservation (C-001):** `csv = "1.4"` (MSRV `1.73`) and `calamine = "0.38"` (transitively resolving `zip 9.8.2`, MSRV `1.89`) advance root `[workspace.package].rust-version` from `"1.88"` to `"1.89"` (UI `[package].rust-version` stays `"1.95"`). Any newly unlocked MSRV-1.89 Clippy lints across root crates are fixed in `T-001`. `.crbd` `FORMAT_VERSION` remains `2`.
- **Zero checked-in binary fixtures:** `.xlsx` unit/integration tests synthesize valid minimal OOXML `.xlsx` zip archives in pure Rust via `zip::ZipWriter`.
- **Safe `.xlsx` & CSV value/formula semantics (C-003):**
  - CSV and XLSX both guard `f.is_finite()` (CSV keeps non-finite strings like `"NaN"`/`"inf"` as `CellValue::Text` matching `engine.rs:1023`; XLSX maps non-finite `Data::Float` to `CellValue::Error(CellError::Num)`).
  - XLSX merges `worksheet_range` (`used_cells` + `val_range.start()`) and `worksheet_formula` (`used_cells` + `fml_range.start()`) independently via `cells.entry(addr)` so formula-only cells (without cached `<v>`) are never dropped.
  - XLSX literal `Data::String(s)` starting with `=` (no `<f>`) is stored with `source: None` and `value: CellValue::Text(s)` so `rebuild_dag` never misparses a literal Excel string as a live formula.
  - `CellErrorType` maps cleanly: `Div0` → `DivZero`, `Ref` → `Ref`, `Num` → `Num`, `Value` → `Value`, and `NA`/`Name`/`Null`/`GettingData` → `CellError::Lang(e.to_string())` (C-006).
- **Engine evaluation & isolation (C-002):**
  - `import_*` clears `self.native`, `self.volatile`, `self.compiled`, and `self.dag`; `replace_sheet_cells` retains only `k.sheet != sheet` in `self.native` and evaluates ONLY the replaced sheet (`evaluate_sheet_formulas(sheet)`) so sibling sheets (e.g., Hex's `=RANDBETWEEN(1,6)`) are never re-evaluated or mutated.
  - `rebuild_dag` marks cycle-closing cells with `CellValue::Error(CellError::Cycle)` (preserving `source`, omitted from `self.compiled`), and skips unparseable Excel formulas while preserving their cached values.
  - The volatile pass unconditionally recomputes the topo-ordered `dirty_closure` of volatile cells on the target sheet(s) so both volatile cells and their downstream dependents settle accurately.
- **Module & lint hygiene (`INT-0002`):** Every `.rs` file under `apps/carbide-ui/src/app/` stays ≤ 1,000 lines, zero `#![allow(...)]`/`#[allow(...)]` attributes under `apps/carbide-ui/src/app/`, and `-D warnings` passes across root, `--features python`, `--features native`, UI native, and UI `wasm32-unknown-unknown`.

## Execution Sequence

### T-001: Add `carbide-store::import` CSV parser, format detection & MSRV 1.89 alignment
- **Intent:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- **Touches:** `Cargo.toml`, `crates/carbide-store/Cargo.toml`, `crates/carbide-store/src/lib.rs`, `crates/carbide-store/src/import.rs`, `Cargo.lock` (plus any root file if MSRV-1.89 unlocks a Clippy lint)
- **Depends on:** (none)
- **Acceptance criterion:** AC-1 (CSV import + format detection, exact MSRV `1.89` alignment, `.crbd` `FORMAT_VERSION` unchanged).
- **Success criterion (EARS):**
  - **WHEN** `import_csv_bytes(bytes, sheet_name)` parses UTF-8 CSV (including optional UTF-8 BOM, quoted fields with commas/newlines, ragged rows, empty cells, integers, finite floats, non-finite `"NaN"`/`"inf"` strings kept as `CellValue::Text`, case-insensitive booleans, text, and `=`-prefixed formulas), **THEN** it **SHALL** return a valid single-sheet `LatticeKind::Square` `Workbook` with A1 addresses, sparse empty omission, typed `CellValue`s for literals, and `source: Some("=...")` for formulas; **WHEN** `bytes` contains invalid UTF-8, **THEN** it **SHALL** return `Err(StoreError::Csv(_))`.
  - **WHEN** `detect_import_format(filename, bytes)` is called with `.xlsx`/`.csv` filenames (any case) or no filename, **THEN** it **SHALL** classify `.xlsx` and `PK\x03\x04` zip payloads as `ImportFormat::Xlsx` and CSV/plain text as `ImportFormat::Csv`.
  - **WHEN** `sheet_name_from_filename(filename)` is called with `None`, empty string, dot-only stem (`".csv"`), or POSIX/Windows paths (`"dir/sales.csv"`, `"C:\\data\\Q1.xlsx"`), **THEN** it **SHALL** strip directory prefixes and extensions cleanly and fall back to `"Sheet1"` when the resulting stem is empty (C-005).
  - **WHEN** `Cargo.toml` sets `rust-version = "1.89"` and `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test -p carbide-store` run, **THEN** all commands **SHALL** exit 0 with zero warnings.
- **Notes:** Promote `carbide-tess = { workspace = true }` to `[dependencies]` of `crates/carbide-store/Cargo.toml`. Add `StoreError::Csv(#[from] csv::Error)` and `StoreError::Xlsx(#[from] calamine::XlsxError)` (C-005).

### T-002: Implement `carbide-store::import::import_xlsx_bytes` via `calamine`
- **Intent:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- **Touches:** `crates/carbide-store/src/import.rs`
- **Depends on:** T-001
- **Acceptance criterion:** AC-1 (multi-sheet `.xlsx` import preserving formulas, typed values, dates, errors, and non-A1 offsets).
- **Success criterion (EARS):**
  - **WHEN** `import_xlsx_bytes(bytes)` parses a valid single- or multi-sheet `.xlsx` archive (including empty sheets where `Range::start() == None`, non-A1 origin ranges such as `B2:C3`, integers, finite/non-finite floats, booleans, inline/shared strings including literal `"=..."` strings without `<f>`, `Data::DateTime`/`DateTimeIso`/`DurationIso`, all `CellErrorType` variants, cached value+formula cells, and formula-only cells without `<v>`), **THEN** it **SHALL** return a `Workbook` with one `LatticeKind::Square` sheet per worksheet in workbook order, merging `worksheet_range` and `worksheet_formula` independently via their own `Range::start()` offsets (C-003), and returning `StoreError` on corrupt or sheet-less inputs.
  - **WHEN** `cargo clippy -p carbide-store --all-targets -- -D warnings` and `cargo test -p carbide-store` run, **THEN** all unit tests (using pure-Rust synthesized `.xlsx` zip archives) **SHALL** pass with zero warnings.
- **Notes:** Verify `Data::DateTime` serial-number conversion and `Data::DateTimeIso` text preservation alongside full `CellErrorType` mapping.

### T-003: Wire `WorkbookEngine` import, formula evaluation, and `replace_sheet_cells`
- **Intent:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- **Touches:** `crates/carbide-formula/src/engine.rs`, `crates/carbide-formula/tests/persistence.rs`
- **Depends on:** T-002
- **Acceptance criterion:** AC-2 (`WorkbookEngine` import methods + topological formula evaluation + volatile/cycle/native safety + sibling preservation + `.crbd` round-trip).
- **Success criterion (EARS):**
  - **WHEN** `WorkbookEngine::import_csv_bytes` or `import_xlsx_bytes` (or native `import_csv`/`import_xlsx`) loads a spreadsheet, **THEN** the engine **SHALL** clear stale `self.native`/`self.volatile`/`self.compiled`/`self.dag`, rebuild the DAG (marking cycle-closing cells `CellValue::Error(CellError::Cycle)` and skipping unparseable Excel formulas while keeping cached values), evaluate all parseable formulas in topological order plus an unconditional volatile `dirty_closure` pass (C-002), recompute dependents on subsequent `set_cell` edits, and round-trip cleanly through `save_bytes`/`open_bytes`.
  - **WHEN** `WorkbookEngine::replace_sheet_cells(sheet_id, name, cells)` replaces a sheet's contents, **THEN** the engine **SHALL** invalidate `self.native` entries for `sheet_id`, rebuild DAG/volatile state, and run sheet-scoped `evaluate_sheet_formulas(sheet_id)` so `sheet_id`'s cells/formulas evaluate cleanly while sibling sheets (Hex, Triangle, Voronoi — including volatile/random cells like `=RANDBETWEEN(1,6)`) **SHALL** remain completely untouched (C-002).
- **Notes:** Add integration tests in `crates/carbide-formula/tests/persistence.rs` for CSV formula eval + `.crbd` round-trip, multi-sheet `.xlsx` eval + graceful unsupported formula degradation, imported volatile formulas + downstream dependents, imported cycles (`CellError::Cycle`), sibling-sheet non-mutation on `replace_sheet_cells`, and a `#[cfg(feature = "native")]` test confirming stale compiled programs are cleared on `replace_sheet_cells`/`import_*`.

### T-004: Wire `carbide-ui` File menu (`Open…`, `Save`, `Import CSV / Excel…`) and native + `wasm32` import flow
- **Intent:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- **Touches:** `apps/carbide-ui/Cargo.lock`, `apps/carbide-ui/src/ribbon.rs`, `apps/carbide-ui/src/app/mod.rs`, `apps/carbide-ui/src/app/demos.rs`, `apps/carbide-ui/src/app/commands.rs`, `apps/carbide-ui/src/app/io.rs`
- **Depends on:** T-003
- **Acceptance criterion:** AC-3 (File menu parity, `.csv`/`.xlsx` picker on native + `wasm32`, repaint/autosave wiring, full stale square-state cleanup, non-square tab preservation, dynamic square tab label, multi-sheet notice, non-fatal error reporting, hygiene bounds).
- **Success criterion (EARS):**
  - **WHEN** the user opens the File menu, **THEN** `Open…`, `Save`, and `Import CSV / Excel…` **SHALL** be enabled and dispatch `RibbonAction::{Open, Save, Import}` (removing the stale disabled `"(Save/Open need engine support)"` placeholder).
  - **WHEN** `handle_import` runs on `wasm32`, **THEN** `spawn_local` **SHALL** clone `egui::Context` and call `ctx.request_repaint()` after storing `(Some(handle.file_name()), bytes)` into `pending_import`; on native, file read errors **SHALL** populate `self.import_error` (C-004).
  - **WHEN** `drain_pending_import` receives valid `.csv` or `.xlsx` bytes, **THEN** it **SHALL** parse via `carbide_store::import`, apply the first sheet into `self.square.sheet_id` via `replace_sheet_cells` (preserving Hex, Triangle, and Voronoi tabs intact), switch to `ActiveSheet::Square`, clear stale square/transient state (`square.formats`, `square_widgets`, `notes`, `metrics`, `history`, `edit`, `format_painter`, `square.formula_drag`/`formula_highlight`, `note_open` if `CellId::Square`, `clipboard.consume_cut()`, refresh `find` if open, cursor to `(0, 0)`), call `self.mark_dirty()`, reflect the imported sheet name on the Square tab, and display a dismissible notice when an `.xlsx` workbook has `> 1` sheet; **WHEN** parsing fails, **THEN** the current workbook **SHALL** remain unchanged and `import_error` **SHALL** surface a dismissible error banner/status (C-004).
  - **WHEN** headless import unit tests in `apps/carbide-ui/src/app/io.rs`, native + `wasm32-unknown-unknown` `fmt`/`clippy -D warnings`/`test`/`build`, root workspace + `--features python`/`native` suites, target-filtered MSRV check (`root = 1.89`, `ui = 1.95`), line-bound check (`<= 1000`), and zero-allow check run, **THEN** all checks **SHALL** exit 0 with zero warnings.
- **Notes:** Place pure/engine-level UI import helper tests inside `apps/carbide-ui/src/app/io.rs` (`#[cfg(test)] mod tests`) so `apps/carbide-ui/src/app/tests.rs` stays well below 1,000 lines (819 lines).
