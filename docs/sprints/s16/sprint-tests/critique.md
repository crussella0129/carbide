# Sprint 16 Test Critique

- **Reviewer:** Independent read-only Test Critic (`research` subagent `15acb071-3c67-4909-a942-cac93154079c`)
- **Verdict:** `proceed-with-caveats`

## Summary
Sprint 16 (`INT-0006`) delivers byte-driven `.csv` and `.xlsx` import cleanly across `carbide-store`, `carbide-formula`, and `apps/carbide-ui` (native + `wasm32`), preserves `.crbd` `FORMAT_VERSION = 2`, satisfies module/lint hygiene (`max = 819 <= 1,000` lines, `0` `#[allow(...)]` attributes under `apps/carbide-ui/src/app/`), honors all 7 Plan Critic concerns (`C-001`–`C-007`), and passes 6/6 GitHub Actions CI jobs. No blocking issues were found; two caveats and three minor items were flagged and resolved in full prior to Loop closeout.

## Findings & Resolution

| ID | Severity | Finding | Resolution |
|----|----------|---------|------------|
| `C-001` | caveat | CSV formula fields with leading whitespace (`" =A1*2"`) stored untrimmed `source`, causing `ExcelLite::parse` (`strip_prefix('=')`) to skip evaluation silently (`crates/carbide-store/src/import.rs:133-138`). | **Fixed:** `import_csv_bytes` now stores `source: Some(field.trim_start().to_string())`; covered by unit test in `crates/carbide-store/src/import.rs` and end-to-end evaluation test in `crates/carbide-formula/tests/persistence.rs`. |
| `C-002` | caveat | Test reports slightly overstated `persistence.rs` sibling-sheet scope (Square+Hex vs. Square+Hex+Triangle+Voronoi) and `Data::Int`/`Data::DurationIso` direct test assertions; `WorkbookEngine::{import_csv, import_xlsx}` path wrappers lacked test callers. | **Fixed:** Added direct `Data::Int` & `Data::DurationIso` assertions in `crates/carbide-store/src/import.rs`, expanded `replace_sheet_cells_evaluates_target_and_preserves_sibling_sheets` in `persistence.rs` to seed and verify all four lattices, added `import_csv_and_xlsx_from_path_and_leading_space_formula` exercising both path wrappers, and synced `unit-tests.md` / `integration-tests.md` / `test-report.md`. |
| `C-003` | minor | Native `handle_import` discarded `ctx` (`let _ = ctx;`) instead of calling `ctx.request_repaint()` after staging import bytes or setting `import_error` (`apps/carbide-ui/src/app/io.rs:244-274`). | **Fixed:** Added `ctx.request_repaint();` on native in `handle_import` after picker completion. |
| `C-004` | minor | Comment at `crates/carbide-formula/tests/persistence.rs:398` mentioned recomputing `C1` after `set_cell`, whereas only `B1` was asserted (pre-existing `set_cell` volatile pass evaluates `self.volatile` directly). | **Fixed:** Clarified the comment at `persistence.rs:398` to match the exact assertion (`// Editing A1 must still recompute volatile B1.`). |
| `C-005` | minor | Unchecked `start_r + rel_r as u32` / `start_c + rel_c as u32` coordinate addition in `import_xlsx_bytes` (`crates/carbide-store/src/import.rs:317-320,338-341`). | **Fixed:** Introduced `checked_xlsx_coord` using `u32::try_from` + `checked_add` + `i32::try_from`, returning `Err(StoreError::Format(_))` on overflow. |

## Acceptance Criteria Audit
- **AC-1 (`carbide-store::import` CSV + `.xlsx`, `.crbd` `FORMAT_VERSION = 2`):** PASS (all `C-001`, `C-002`, `C-005` resolved).
- **AC-2 (`WorkbookEngine` import, DAG/volatile/cycle/native safety, `replace_sheet_cells`, `.crbd` round-trip):** PASS (all `C-002`, `C-004` resolved).
- **AC-3 (`apps/carbide-ui` File menu parity, native + `wasm32`, sibling preservation, non-fatal errors, hygiene):** PASS (`C-003` resolved; max 819 lines, 0 `allow`s).

## Plan Critic Follow-Through (`C-001`..`C-007`)
All seven Plan Critic concerns (`C-001` MSRV 1.89 & `.crbd` v2; `C-002` engine eval/cycle/volatile/native/sibling isolation; `C-003` finite-float/independent `Range::start()`/literal `"=..."` safety; `C-004` wasm repaint/native errors/stale UI reset/`mark_dirty()`/multi-sheet notice; `C-005` `StoreError::{Csv, Xlsx}` & filename fallback; `C-006` `CellErrorType` mapping; `C-007` task anchor sync) verified honored.
