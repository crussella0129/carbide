# INT-0006 — CSV and `.xlsx` spreadsheet import

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0006
- **State:** realized
- **Work evidence:** [Sprint 16 build-plan.md](../sprints/s16/sprint-plans/build-plan.md) (`T-001`–`T-004`)
- **Completion evidence:** [T-001 (sprint 16)](../work/completed-tasks.md#t-001-sprint-16), [T-002 (sprint 16)](../work/completed-tasks.md#t-002-sprint-16), [T-003 (sprint 16)](../work/completed-tasks.md#t-003-sprint-16), [T-004 (sprint 16)](../work/completed-tasks.md#t-004-sprint-16)
- **Code evidence:** [crates/carbide-store/src/import.rs](../../crates/carbide-store/src/import.rs) (`8605d8a`, `f2abe8b`, `43ef642`), [crates/carbide-store/src/lib.rs](../../crates/carbide-store/src/lib.rs) (`8605d8a`), [crates/carbide-formula/src/engine.rs](../../crates/carbide-formula/src/engine.rs) (`5598452`), [apps/carbide-ui/src/ribbon.rs](../../apps/carbide-ui/src/ribbon.rs) (`f90cfdd`), [apps/carbide-ui/src/app/io.rs](../../apps/carbide-ui/src/app/io.rs) (`f90cfdd`, `43ef642`), [apps/carbide-ui/src/app/mod.rs](../../apps/carbide-ui/src/app/mod.rs) (`f90cfdd`), [apps/carbide-ui/src/app/commands.rs](../../apps/carbide-ui/src/app/commands.rs) (`f90cfdd`), [apps/carbide-ui/src/app/demos.rs](../../apps/carbide-ui/src/app/demos.rs) (`f90cfdd`)
- **Test evidence:** [Sprint 16 test-report.md](../sprints/s16/sprint-tests/test-report.md), [Sprint 16 critique.md](../sprints/s16/sprint-tests/critique.md), [crates/carbide-formula/tests/persistence.rs](../../crates/carbide-formula/tests/persistence.rs) (`5598452`, `43ef642`), [GitHub Actions CI run #45949651858](https://github.com/crussella0129/carbide/actions/runs/45949651858) (6/6 jobs `success` on `43ef642`)
- **Documentation evidence:** [Sprint 16 research-report.md](../sprints/s16/sprint-research/research-report.md), [Sprint 16 build-plan.md](../sprints/s16/sprint-plans/build-plan.md), [Sprint 16 test-plan.md](../sprints/s16/sprint-plans/test-plan.md)

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
- 2026-11-27: `active` → `realized` (Sprint 16 Test & Loop closeout; Test Critic `proceed-with-caveats` with `C-001`–`C-005` resolved at `43ef642`; CI run `#45949651858` 6/6 `success`).
