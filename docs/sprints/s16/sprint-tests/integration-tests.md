# Sprint 16 Integration Tests

## Scope
- `verify_engine_import_and_eval` (`carbide-formula` integration suites across default, `--features native`, and `--features python`, `T-003`)

## Execution & Results (final verified commit `43ef642`)
1. **Default workspace integration suites (`cargo test --workspace`):**
   - `crates/carbide-formula/tests/persistence.rs`: **8 passed, 0 failed** (3 existing `.crbd` round-trip tests + 5 new import tests):
     - `import_csv_bytes_evaluates_formulas_recomputes_and_roundtrips_crbd` — verifies immediate topological evaluation on CSV import, downstream recomputation after `set_cell`, and clean `.crbd` v2 `save_bytes`/`open_bytes` round-trip.
     - `import_csv_bytes_evaluates_volatile_dependents_and_marks_cycles` — verifies volatile `INDIRECT`/`OFFSET` cells AND their downstream dependents evaluate on import, and cycle-closing cells are marked `CellValue::Error(CellError::Cycle)`.
     - `import_xlsx_bytes_evaluates_multisheet_and_preserves_cached_unsupported_formula` — verifies multi-sheet `.xlsx` formula evaluation plus graceful cached-value preservation on unsupported Excel formula syntax.
     - `replace_sheet_cells_evaluates_target_and_preserves_sibling_sheets` — verifies replacing the Square sheet evaluates its formulas while sibling Hex (including `=RANDBETWEEN(1,1000000)`), Triangle, and Voronoi sheets stay untouched.
     - `import_csv_and_xlsx_from_path_and_leading_space_formula` — verifies path-based `WorkbookEngine::import_csv` and `WorkbookEngine::import_xlsx` plus leading-whitespace CSV formula evaluation (`"10, =A1*2"` → `20.0`, per Test Critic `C-001`/`C-002`).
   - `dag_recompute` (3), `hex_conway` (3), `reference_examples` (35), `scale` (4), `transpile_differential` (1), `transpile_fuzz` (4), `carbide-tess/tests/lattice_geometry.rs` (10): all **PASS** (0 failed).
2. **Native compile pipeline (`--features native`):**
   - `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings` — **PASS** (0 warnings)
   - `cargo test -p carbide-formula --features native` — **PASS** (**237 passed, 0 failed**): 176 unit + 61 integration, including **9/9** in `persistence.rs` (adding `replace_sheet_cells_clears_stale_native_programs` confirming stale `self.native` entries are invalidated on `replace_sheet_cells`) and `native_differential` (2/2).
3. **Python engine pipeline (`--features python`, Python 3.13.11):**
   - `cargo clippy -p carbide-formula --features python --all-targets -- -D warnings` — **PASS** (0 warnings)
   - `cargo test -p carbide-formula --features python` — **PASS** (**246 passed, 0 failed**): 185 unit + 61 integration (including `cross_engine` 3/3 and `persistence` 8/8).
