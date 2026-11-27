# Sprint 16 Test Report

## Summary
- **Overall verdict:** PASS (all default workspace + `--features native` + `--features python` + UI native + `wasm32-unknown-unknown` builds, clippy `-D warnings`, unit/integration tests, MSRV alignment, Test Critic `C-001`–`C-005` resolutions, and **6/6 GitHub Actions CI jobs `success`** on run [#45949651858](https://github.com/crussella0129/carbide/actions/runs/45949651858)).
- **Intent verified:** [INT-0006 — CSV and `.xlsx` spreadsheet import](../../../intents/INT-0006-csv-and-xlsx-import.md).
- **Verified commit:** `43ef6423e91a7f1076f2e341fb4e1e140eacf3f2` on `dev` (incorporating `T-001` `8605d8a`, `T-002` `f2abe8b`, `T-003` `5598452`, `T-004` `f90cfdd`, the clean `origin/dev` Dependabot merge `2ee93cb`, and Test Critic `C-001`–`C-005` follow-up `43ef642`).

## Acceptance Criteria Traceability
| Acceptance Criterion | Status | Evidence |
|----------------------|--------|----------|
| AC-1: `carbide-store::import` provides byte-driven `import_csv_bytes` and `import_xlsx_bytes` without altering `.crbd` `FORMAT_VERSION` (`2`) | PASS | `crates/carbide-store/src/import.rs` implements `ImportFormat`, `detect_import_format`, `sheet_name_from_filename`, `import_csv_bytes` (`csv 1.4` + trimmed leading-space formulas), and `import_xlsx_bytes` (`calamine 0.38` + checked coords + independent `Range::start()` formula/value merge, finite-float guard, literal `"=..."` safety, `DateTime`/`DateTimeIso`/`DurationIso`, full `CellErrorType` mapping); 16/16 `carbide-store` tests green with `.crbd` `FORMAT_VERSION = 2` unchanged; [unit-tests.md](unit-tests.md) |
| AC-2: `WorkbookEngine` provides `import_csv_bytes`, `import_xlsx_bytes`, `import_csv`, `import_xlsx`, and `replace_sheet_cells` with DAG rebuild, volatile tracking, topological evaluation, and `.crbd` round-trip | PASS | `crates/carbide-formula/src/engine.rs` clears stale `native`/`volatile`/`compiled`/`dag`, marks cycle-closing cells `CellError::Cycle`, evaluates topological + unconditional volatile `dirty_closure` passes, isolates `replace_sheet_cells` to the target sheet so sibling sheets (Hex/Triangle/Voronoi) stay untouched; 343/343 default workspace, 237/237 `--features native` (9/9 `persistence.rs`), and 246/246 `--features python` tests green; [integration-tests.md](integration-tests.md) |
| AC-3: `apps/carbide-ui` wires `File -> Open…`, `Save`, and `Import CSV / Excel…` on native + `wasm32`, preserves non-square sheets, and surfaces errors non-fatally | PASS | `apps/carbide-ui/src/ribbon.rs` + `apps/carbide-ui/src/app/{mod,demos,commands,io}.rs` enable File menu actions, stage `.csv`/`.xlsx` via `rfd` (`ctx.request_repaint()` on native & `wasm32`), reset all stale square/transient state, preserve Hex/Triangle/Voronoi tabs, update the Square tab label dynamically, show dismissible multi-sheet notice & error banners, and mark dirty for autosave; 318/318 native UI tests green, `wasm32-unknown-unknown` clippy `-D warnings` & build clean, 6/6 CI jobs green on run [#45949651858](https://github.com/crussella0129/carbide/actions/runs/45949651858); [unit-tests.md](unit-tests.md), [e2e-tests.md](e2e-tests.md) |

## Hygiene & Compatibility Checks
- `apps/carbide-ui/src/app/*.rs`: max **819 lines** (`<= 1,000`; `io.rs` = 703, `mod.rs` = 701), zero `#![allow(...)]`/`#[allow(...)]` attributes.
- Target-filtered MSRV: `Cargo.toml` = `"1.89"`, `apps/carbide-ui/Cargo.toml` = `"1.95"` (exact match).
- `.crbd` `FORMAT_VERSION` unchanged at `2`; `zip` deduplicated to a single `9.8.2` line in both lockfiles.
- Book & substrate checks: `check-book.sh` → `valid v2 Book (6 intent chapters)`; `check-substrate.sh` → `substrate-complete`.

## Detailed Reports
- [Unit tests](unit-tests.md)
- [Integration tests](integration-tests.md)
- [End-to-end tests](e2e-tests.md)
