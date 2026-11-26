# Sprint 13 Test Report

## Intent Verification
| Intent | Acceptance criterion | EARS / tests | Result | Intent evidence update |
|--------|----------------------|---------------|--------|------------------------|
| [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) | AC-1: No single file under `apps/carbide-ui/src/app/` exceeds 1,000 lines | T-001..T-004 / `verify_module_line_bounds` (15 files, max 839 lines in `tests.rs`, max non-test 761 lines in `render_square.rs`; `app.rs` removed) | pass | Test evidence links this report; eligible for `realized` in Loop |
| [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) | AC-2: Public crate API (`carbide_ui::CarbideApp`), `pub(crate)` snapshot API, and all 308 UI + 331 workspace tests unchanged and green across native and `wasm32-unknown-unknown` | T-001..T-004 / `verify_public_api_unchanged`, `verify_ui_tests_and_wasm32` (308/308 UI + wasm32 0 warnings), `verify_workspace_regression` (331/331), `verify_github_actions_ci_on_dev` (run `45872819726` 6/6 jobs `success`) | pass | Test evidence links this report; eligible for `realized` in Loop |
| [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) | AC-3: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with zero new `#[allow(...)]` suppressions | T-001..T-004 / `verify_ui_fmt_clippy_no_allows` (0 warnings, 0 `#!?[allow(` matches), `verify_workspace_regression`, `verify_github_actions_ci_on_dev` | pass | Test evidence links this report; eligible for `realized` in Loop |

## Summary
- Unit tests: 576 passed / 0 failed / 576 total (`carbide-ui`: 308, `carbide-core`: 8, `carbide-formula`: 174, `carbide-store`: 9, `carbide-tess`: 77) + 3 module/API/hygiene checks passed
- Integration tests: 63 passed / 0 failed / 63 total (`dag_recompute`: 3, `hex_conway`: 3, `persistence`: 3, `reference_examples`: 35, `scale`: 4, `transpile_differential`: 1, `transpile_fuzz`: 4, `lattice_geometry`: 10)
- E2E tests: 1 passed / 0 failed / 1 total (`verify_github_actions_ci_on_dev` — all 6 GitHub Actions jobs `success` on `dev`)
- CI status: green

## CI Confirmation
- **Head SHA:** `2c65eaa6418e055a9d197ff5237e8b64793b05b3`
- **CI run:** `45872819726` — https://github.com/crussella0129/carbide/actions/runs/45872819726
- **Conclusion:** success
- **Confirmations:** [unit-tests.md](unit-tests.md), [integration-tests.md](integration-tests.md), [e2e-tests.md](e2e-tests.md)

## Failures
None.

## Technical Debt Identified
- [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) — Unblocked by `INT-0002`: per-lattice renderers and commands are now cleanly separated for Voronoi format/notes/copy-paste/`Selection.extra` parity work.
- Minor pre-existing wasm32 dialog filter bug observed (intentionally left untouched to preserve Sprint 13's zero-behavior-change contract): `apps/carbide-ui/src/app/io.rs` `#[cfg(target_arch = "wasm32")]` arms in `handle_save` / `handle_open` still pass `&["tscl"]` instead of `&["crbd"]` / `&["crbd", "tscl"]` (unlike the native arms). Good candidate to include in the next UI sprint (`INT-0003`).
- [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) — Dependency and toolchain modernization (`egui`/`eframe`, `rfd`, `pyo3`, Cargo Dependabot ecosystems).
- [INT-0005](../../../intents/INT-0005-post-rename-product-roadmap.md) — Post-rename feature roadmap awaiting prioritization.

## Coverage Observations
- Every EARS clause and all 3 acceptance criteria of `INT-0002` were verified after `T-004` (`d528699`) and confirmed across all 6 GitHub Actions CI jobs (`45872819726`) on `2c65eaa`.
- Zero `#[allow(...)]` or `#![allow(...)]` attributes exist anywhere under `apps/carbide-ui/src/app/`, and `mod.rs` carries zero unused glob imports across both `--lib`/`wasm32` and `--tests` targets.
