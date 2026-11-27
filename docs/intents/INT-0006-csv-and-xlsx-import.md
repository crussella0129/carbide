# INT-0006 — CSV and `.xlsx` spreadsheet import

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0006
- **State:** active
- **Work evidence:** [Sprint 16 build-plan.md](../sprints/s16/sprint-plans/build-plan.md) (`T-001`–`T-004`)
- **Completion evidence:** none
- **Code evidence:** none
- **Test evidence:** none
- **Documentation evidence:** none

## Intent
Deliver the first post-rename product feature deferred in ADR-015 and tracked in [INT-0005](INT-0005-post-rename-product-roadmap.md): byte-driven, native + `wasm32`-compatible import of `.csv` and `.xlsx` spreadsheets (`csv` + `calamine` crates) across `carbide-store`, `carbide-formula` (`WorkbookEngine`), and `apps/carbide-ui` (including wiring the File menu's `Open…`, `Save`, and `Import CSV / Excel…` actions).

## Acceptance criteria
1. `carbide-store::import` provides byte-driven `import_csv_bytes` (via `csv`, handling UTF-8 BOM, quoted fields, ragged rows, typed literals, and `=`-prefixed formulas) and `import_xlsx_bytes` (via `calamine`, preserving all worksheets in order, importing `=`-prefixed formulas via `worksheet_formula` alongside typed values `Int`/`Float`/`Bool`/`String`/`DateTime`/`Error`), producing valid `LatticeKind::Square` `Workbook`s without altering `.crbd` `FORMAT_VERSION` (`2`).
2. `WorkbookEngine` provides `import_csv_bytes`, `import_xlsx_bytes`, `import_csv`, `import_xlsx`, and `replace_sheet_cells`, rebuilding the DAG, tracking volatile formulas, and evaluating all imported formulas in topological order so imported workbooks are immediately live and `.crbd` round-trippable.
3. `apps/carbide-ui` wires `File -> Open…`, `File -> Save`, and `File -> Import CSV / Excel…` (removing the stale disabled File menu placeholder), supports `.csv` and `.xlsx` import on both native and `wasm32-unknown-unknown` (0 warnings under `-D warnings`), preserves non-square lattice sheets while populating the square sheet with imported data, and surfaces import parse errors non-fatally.

## Rationale
Users evaluating Carbide after the rename need a direct way to bring real tabular data and existing Excel workbooks into the app without hand-typing cells. Byte-driven import in `carbide-store` keeps native desktop, headless/CLI, and WebAssembly front-ends on one shared implementation.

## Alternatives
- **UI-only CSV parser without `calamine` or store integration:** duplicates parsing logic in `carbide-ui`, leaves `carbide-formula`/`carbide-cli` without import APIs, and omits `.xlsx` support called out in ADR-015.

## Consequences
- Adds `csv = "1.4"` (MSRV `1.73`) and `calamine = "0.38"` (transitively resolving `zip 9.8.2`, MSRV `1.89`) to `crates/carbide-store/Cargo.toml`; root `[workspace.package].rust-version` advances from `"1.88"` to `"1.89"` (UI MSRV remains `"1.95"`).
- `.crbd` `FORMAT_VERSION` remains `2`.

## Transition history
- 2026-11-27: split from `INT-0005` item 1 (ADR-015 post-rename item) as `proposed` during Sprint 16 Research Phase.
- 2026-11-27: `proposed` → `planned` (Sprint 16 Plan Phase: `T-001`–`T-004` in `docs/sprints/s16/sprint-plans/build-plan.md`).
- 2026-11-27: `planned` → `active` (Sprint 16 Build Phase started).
