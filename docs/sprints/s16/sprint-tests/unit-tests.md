# Sprint 16 Unit Tests

## Scope
- `verify_store_csv_and_xlsx_import` (`carbide-store`, `T-001` & `T-002`)
- `verify_ui_import_wasm32_and_hygiene` (`apps/carbide-ui`, `T-004`)
- Full workspace unit suites (`carbide-core`, `carbide-formula`, `carbide-store`, `carbide-tess`, `carbide-ui`)

## Execution & Results (final verified commit `43ef642`)
1. **Root workspace formatting, lint & unit tests:**
   - `cargo fmt --all -- --check` — **PASS** (exit 0)
   - `cargo clippy --workspace --all-targets -- -D warnings` — **PASS** (exit 0, 0 warnings)
   - `cargo test --workspace` — **PASS** (**343 passed, 0 failed**, including **16/16** `carbide-store` unit tests, **8/8** `carbide-core`, **174/174** `carbide-formula` unit tests, and **77/77** `carbide-tess` unit tests).
   - `carbide-store::import` coverage (`crates/carbide-store/src/import.rs`):
     - CSV: UTF-8 BOM stripping, invalid UTF-8 returning `Err(StoreError::Csv(_))`, quoted fields with commas & embedded newlines, ragged rows, sparse empty-cell omission, typed literals (`Integer`/`Number`/`Bool`/`Text`), non-finite `"NaN"`/`"inf"` strings preserved as `CellValue::Text`, `=`-prefixed formulas (including leading-whitespace formulas trimmed via `trim_start()`, per Test Critic `C-001`), `detect_import_format` (extension + `PK\x03\x04` magic bytes), and `sheet_name_from_filename` (`None`, empty, `".csv"`, POSIX `/` & Windows `\` paths → `"Sheet1"` fallback).
     - XLSX (pure-Rust synthesized OOXML zip archives + direct `cell_from_excel_data` tests): single- and multi-sheet workbooks in workbook order, empty sheet (`Range::start() == None`), non-A1 origin offset (`B2:C3`) with overflow-checked `checked_xlsx_coord` (`C-005`), `Data::Int`/`Float` (finite + non-finite guard → `CellError::Num`)/`Bool`/`String` (including literal `"=..."` without `<f>` stored with `source: None`)/`DateTime`/`DateTimeIso`/`DurationIso`, all 8 `CellErrorType` variants (`Div0`, `Ref`, `Num`, `Value`, `NA`, `Name`, `Null`, `GettingData`), cached value+formula cells AND formula-only cells without `<v>` merged independently via `Range::start()`, corrupt bytes returning `StoreError`, and `.crbd` `FORMAT_VERSION == 2` round-trip tests unchanged.
2. **UI crate native + `wasm32-unknown-unknown` + hygiene (`T-004`):**
   - `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check` — **PASS** (exit 0)
   - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` — **PASS** (exit 0, 0 warnings)
   - `cargo test --manifest-path apps/carbide-ui/Cargo.toml` — **PASS** (**318 passed, 0 failed**), including 3 new headless import tests in `apps/carbide-ui/src/app/io.rs`:
     - `app::io::tests::apply_imported_csv_updates_square_sheet_and_preserves_siblings`
     - `app::io::tests::apply_imported_multisheet_xlsx_reports_total_sheets_and_evaluates_formulas`
     - `app::io::tests::apply_imported_corrupt_bytes_leaves_engine_untouched`
   - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown --all-targets -- -D warnings` — **PASS** (exit 0, 0 warnings)
   - `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` — **PASS** (exit 0)
   - Module line bounds (`wc -l apps/carbide-ui/src/app/*.rs`): max **819 lines** (`tests.rs`; `io.rs` = 703, `mod.rs` = 701; all 15 files `<= 1,000`).
   - Lint suppression audit (`! grep -rnE '#!?\[allow\(' apps/carbide-ui/src/app/`): **0 matches** (exit 0).
