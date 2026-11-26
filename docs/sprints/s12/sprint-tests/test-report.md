# Sprint 12 Test Report

## Intent Verification
| Intent | Acceptance criterion | EARS / tests | Result | Intent evidence update |
|--------|----------------------|---------------|--------|------------------------|
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-1: Workspace fmt, clippy, native clippy, and unit/integration tests green | T-002 / `verify_workspace_fmt_clippy_test` (331/331 tests, 0 warnings) + GitHub CI (`lint`, `test` ubuntu/windows, `native`, `python`) | pass | Test evidence links this report; eligible for `realized` in Loop |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-2: UI fmt, clippy (`-D warnings`), tests, and wasm32 build green with zero warnings | T-001 / `verify_ui_fmt_clippy_test_wasm` (308/308 tests, 0 warnings native + wasm32) + GitHub CI (`webui`) | pass | Test evidence links this report; eligible for `realized` in Loop |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-3: Zero `#[allow(dead_code)]` in `apps/carbide-ui/src/app.rs` | T-001 / `verify_no_dead_code_allow_in_app` (0 matches) | pass | Test evidence links this report |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-4: All 3 ADR-015 repo URLs updated to `crussella0129/carbide` | T-002 / `verify_adr015_repo_urls` (0 `tescellate` URLs, 3 `carbide` URLs) | pass | Test evidence links this report |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-5: `.github/workflows/ci.yml` triggers `push` on both `main` and `dev` | T-003 / `verify_ci_push_branches` + live `push` run `45866040577` on `dev` | pass | Test evidence links this report |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-6: `README.md`, `CLAUDE.md`, `carbide-cli/src/main.rs`, and About text accurate with zero behavior regression | T-002, T-004 / `verify_adr015_repo_urls`, `verify_docs_and_metadata_accuracy`, `verify_workspace_fmt_clippy_test`, `verify_ui_fmt_clippy_test_wasm` | pass | Test evidence links this report |

## Summary
- Unit tests: 576 passed / 0 failed / 576 total (`carbide-ui`: 308, `carbide-core`: 8, `carbide-formula`: 174, `carbide-store`: 9, `carbide-tess`: 77) + 5 static/CLI verification checks passed
- Integration tests: 63 passed / 0 failed / 63 total (`dag_recompute`: 3, `hex_conway`: 3, `persistence`: 3, `reference_examples`: 35, `scale`: 4, `transpile_differential`: 1, `transpile_fuzz`: 4, `lattice_geometry`: 10)
- E2E tests: 1 passed / 0 failed / 1 total (`verify_github_actions_ci_on_dev` — all 6 GitHub Actions jobs `success` on `dev`)
- CI status: green

## CI Confirmation
- **Head SHA:** `70e25fcfcf7d7ecafe495ab173e436de8ff9ba32`
- **CI run:** `45866040577` — https://github.com/crussella0129/carbide/actions/runs/45866040577
- **Conclusion:** success
- **Confirmations:** [unit-tests.md](unit-tests.md), [integration-tests.md](integration-tests.md), [e2e-tests.md](e2e-tests.md)

## Failures
None. During initial post-`T-004` verification, running the documented `cargo run --bin carbide-cli` caught that `crates/carbide-cli/Cargo.toml` defines `[[bin]] name = "carbide-core"`, so `CLAUDE.md` was corrected to `cargo run -p carbide-cli` in commit `70e25fc` and all checks re-passed cleanly.

## Technical Debt Identified
- [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) — `apps/carbide-ui/src/app.rs` remains a 7,433-line monolith; deferred for user prioritization.
- [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) — Voronoi formatting, notes, copy/paste, and `Selection.extra` operational parity (ADR-013 deferral).
- [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) — Dependency and toolchain refresh (`egui 0.29`, `rfd 0.14`, `pyo3 0.23`, `hashbrown 0.14`, `glam 0.27`, Dependabot `cargo` ecosystems, MSRV review).
- [INT-0005](../../../intents/INT-0005-post-rename-product-roadmap.md) — Deferred post-v161 product roadmap items awaiting user prioritization.
- Remote hygiene (human-gated): 72 legacy merged feature branches remain on `origin`, and 4 May-2026 PNG screenshots remain in `examples/`.

## Coverage Observations
- Every EARS clause and all 6 acceptance criteria of `INT-0001` were verified after the final code/doc commit (`70e25fc`), satisfying Plan Critic C-003.
- GitHub Actions confirmed cross-OS (`ubuntu-latest`, `windows-latest`) and optional-feature (`--features native`, `--features python`, `wasm32-unknown-unknown`) coverage in addition to local Windows verification.
