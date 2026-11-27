Finalized - DO NOT EDIT

# Sprint 16 Test Plan

## Intent Traceability
| Intent | Acceptance criterion | Build task / EARS clause | Verification |
|--------|----------------------|--------------------------|--------------|
| [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md) | AC-1: `carbide-store::import` provides byte-driven `import_csv_bytes` and `import_xlsx_bytes` without altering `.crbd` `FORMAT_VERSION` (`2`) | T-001, T-002 / WHEN CSV (BOM, invalid UTF-8, quotes, ragged rows, `NaN`/`inf`, typed literals, formulas) and synthesized multi-sheet `.xlsx` (empty sheets, offset ranges, formula-only cells, literal `"=..."` strings, dates, all `CellErrorType`s) are imported THEN valid Square `Workbook`s are produced and corrupt inputs return `StoreError` | `verify_store_csv_and_xlsx_import` |
| [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md) | AC-2: `WorkbookEngine` imports CSV/`.xlsx`, rebuilds DAG + volatile tracking, evaluates formulas topologically, supports `replace_sheet_cells`, and round-trips `.crbd` | T-003 / WHEN CSV/`.xlsx` loads into `WorkbookEngine` or replaces a sheet THEN formulas, volatile closures, and cycles evaluate properly, unsupported Excel formulas degrade gracefully, `self.native` clears, sibling sheets stay untouched, and `.crbd` round-trips | `verify_engine_import_and_eval` |
| [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md) | AC-3: `apps/carbide-ui` wires File menu `Open…`/`Save`/`Import CSV / Excel…`, native + `wasm32` import flow, non-square tab preservation, and non-fatal error display | T-004 / WHEN UI import helpers, native + `wasm32-unknown-unknown` (`-D warnings`), target-filtered MSRV (`root = 1.89`, `ui = 1.95`), line bounds (`<= 1000`), zero-allow checks, and all 6 CI jobs run THEN all pass with 0 warnings | `verify_ui_import_wasm32_and_hygiene`, `verify_msrv_and_github_ci` |

## Execution Order Guarantee
All verification gates below run in full after `T-004` completes (in addition to incremental checks after each task `T-001`–`T-004`).

## Unit Tests
### Store CSV & `.xlsx` import (`carbide-store`, T-001, T-002)
- **Intent:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- `verify_store_csv_and_xlsx_import`: `cargo test -p carbide-store` covering:
  - CSV: UTF-8 BOM stripping, invalid UTF-8 returning `Err(StoreError::Csv(_))`, quoted commas/newlines, ragged rows, empty/blank sparse omission, typed literals (`Integer`/`Number`/`Bool`/`Text`), non-finite `"NaN"`/`"inf"` strings kept as `CellValue::Text`, `=`-prefixed formulas, `detect_import_format` (extension + `PK\x03\x04` magic bytes), and `sheet_name_from_filename` (`None`, empty, `".csv"`, POSIX `/` & Windows `\` paths → `"Sheet1"` fallback).
  - XLSX (pure-Rust synthesized zip archives): single- and multi-sheet workbooks, empty sheet (`Range::start() == None`), non-A1 origin offset (`B2:C3`), `Int`/`Float` (finite + non-finite guard)/`Bool`/`String` (including literal `"=..."` without `<f>` stored with `source: None`)/`DateTime`/`DateTimeIso`, all `CellErrorType` variants (`Div0`, `Ref`, `Num`, `Value`, `NA`, `Name`, `Null`, `GettingData`), cached value+formula cells AND formula-only cells without `<v>`, corrupt bytes returning `StoreError`, and `.crbd` `FORMAT_VERSION == 2` regression tests.

### UI crate native + `wasm32` + import helper tests + hygiene (T-004)
- **Intent:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- `verify_ui_import_wasm32_and_hygiene`:
  - Headless unit tests in `apps/carbide-ui/src/app/io.rs` covering `apply_imported_bytes` (CSV + multi-sheet `.xlsx` selection, sheet-name propagation, multi-sheet notice text, and non-fatal error return leaving the engine untouched).
  - `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`
  - `cargo test --manifest-path apps/carbide-ui/Cargo.toml`
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown --all-targets -- -D warnings`
  - `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown`
  - Verify `max(lines) <= 1000` for every `*.rs` under `apps/carbide-ui/src/app/` and zero `#!?[allow(` attributes under `apps/carbide-ui/src/app/`.

## Integration Tests
### Engine import, topological evaluation, volatile/cycle/native safety & `.crbd` round-trip (T-003)
- **Intents:** [INT-0006](../../../intents/INT-0006-csv-and-xlsx-import.md)
- `verify_engine_import_and_eval`: `cargo test --workspace` (specifically `crates/carbide-formula/tests/persistence.rs` + full workspace) plus `--features python` and `--features native`, verifying:
  - `WorkbookEngine::import_csv_bytes` evaluates formulas immediately, recomputes on `set_cell`, handles imported cycles (`CellError::Cycle`), evaluates volatile formulas (`INDIRECT`/`OFFSET`) AND their downstream dependents, and round-trips via `save_bytes`/`open_bytes`.
  - `WorkbookEngine::import_xlsx_bytes` evaluates multi-sheet `.xlsx` formulas and gracefully keeps cached values on unsupported Excel formula syntax.
  - `WorkbookEngine::replace_sheet_cells` updates the target Square sheet and evaluates its formulas while keeping sibling Hex/Triangle/Voronoi sheets (including `=RANDBETWEEN(1,6)`) completely untouched, and clears stale `self.native` compiled programs under `--features native`.

## End-to-End Tests
- **Status:** possible
- `verify_msrv_and_github_ci`: verify target-filtered MSRV alignment (`root = 1.89`, `ui = 1.95` via normalized `cargo metadata --filter-platform`), push `dev` and confirm all 6 GitHub Actions CI jobs conclude `success` via `gh run watch`, plus `check-substrate.sh` (`substrate-complete`) and `check-book.sh` (`valid v2 Book`).
