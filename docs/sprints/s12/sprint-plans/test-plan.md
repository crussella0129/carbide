Finalized - DO NOT EDIT

# Sprint 12 Test Plan

## Intent Traceability
| Intent | Acceptance criterion | Build task / EARS clause | Verification |
|--------|----------------------|--------------------------|--------------|
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-1: Workspace fmt, clippy, native clippy, and unit/integration tests green | T-002 / WHEN workspace fmt/clippy/native-clippy/tests run THEN exit 0 with 331 passing tests and zero warnings | `verify_workspace_fmt_clippy_test` |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-2: UI fmt, clippy (`-D warnings`), tests, and wasm32 build green with zero warnings | T-001 / WHEN UI fmt/clippy/test/wasm32 build run THEN exit 0 with 308 passing tests and zero warnings | `verify_ui_fmt_clippy_test_wasm` |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-3: Zero `#[allow(dead_code)]` in `apps/carbide-ui/src/app.rs` | T-001 / WHEN `app.rs` is inspected for `#[allow(dead_code)]` THEN zero occurrences remain | `verify_no_dead_code_allow_in_app` |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-4: All 3 ADR-015 repo URLs updated to `crussella0129/carbide` | T-002 / WHEN `git grep` checks `Cargo.toml`, `CLAUDE.md`, `app.rs` THEN zero `tescellate` URLs and three `carbide` URLs | `verify_adr015_repo_urls` |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-5: `.github/workflows/ci.yml` triggers `push` on both `main` and `dev` | T-003 / WHEN `ci.yml` `on.push.branches` is inspected THEN both `main` and `dev` present with all 5 jobs intact | `verify_ci_push_branches` |
| [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md) | AC-6: `README.md`, `CLAUDE.md`, `carbide-cli/src/main.rs`, and About text accurate with zero behavior regression | T-002, T-004 / WHEN docs, CLI stub, and About text are checked AND full workspace/UI suites run THEN zero stale claims and zero regressions | `verify_adr015_repo_urls`, `verify_docs_and_metadata_accuracy`, `verify_workspace_fmt_clippy_test`, `verify_ui_fmt_clippy_test_wasm` |

## Execution Order Guarantee (Critic C-003)
Because `T-002` edits `apps/carbide-ui/src/app.rs` after `T-001` and `T-004` edits `CLAUDE.md` after `T-002`, **all** verification gates below are executed in the Test Phase **after `T-004` completes**, guaranteeing final post-edit coverage on every shared file.

## Unit Tests
### T-001 unit & lint verification (re-run after T-004)
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- `verify_ui_fmt_clippy_test_wasm`: run `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`, `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path apps/carbide-ui/Cargo.toml` (all 308 existing unit tests pass with zero compiler warnings), and `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` (zero warnings).
- `verify_no_dead_code_allow_in_app`: `git grep -n "allow(dead_code)" apps/carbide-ui/src/app.rs` returns zero matches.
- Stubs: none (pure compiler/clippy + existing 308 unit tests).

### T-002 metadata & URL verification (re-run after T-004)
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- `verify_adr015_repo_urls`: `git grep -i "crussella0129/tescellate" -- Cargo.toml CLAUDE.md apps/carbide-ui/src/app.rs` returns zero matches; `git grep -n "crussella0129/carbide" -- Cargo.toml CLAUDE.md apps/carbide-ui/src/app.rs` matches all 3 sites; `crates/carbide-cli/src/main.rs` contains no `becomes` rename artifact; About text in `apps/carbide-ui/src/app.rs` lists squares, hexagons, triangles, and Voronoi.

### T-003 CI workflow verification
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- `verify_ci_push_branches`: inspect `.github/workflows/ci.yml` confirming `branches: [main, dev]` under `on.push` and all 5 jobs (`lint`, `test`, `native`, `python`, `webui`) preserved unchanged.

### T-004 documentation verification
- **Intent:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- `verify_docs_and_metadata_accuracy`:
  - `git grep -nE "Status: Phase 0|Nothing user-visible works yet|apps/desktop|Node\.js 20|JSON-RPC IPC|Tauri|prettier|eslint|npm install|npm run dev" -- README.md CLAUDE.md` returns zero matches.
  - `git grep -n "docs/history/decisions-legacy.md" CLAUDE.md` and `git grep -n "native" CLAUDE.md` both match.
  - `cargo run --bin carbide-cli` exits 0 and prints the documented GUI launch command.

## Integration Tests
### Full workspace & UI integration verification (re-run after T-004)
- **Intents:** [INT-0001](../../../intents/INT-0001-reentry-ci-restoration-and-hygiene.md)
- `verify_workspace_fmt_clippy_test`: run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings`, and `cargo test --workspace` (all 331 workspace tests incl. `dag_recompute`, `hex_conway`, `persistence`, `reference_examples`, `scale`, `transpile_differential`, `transpile_fuzz`, `lattice_geometry`, and `carbide-store` `.tscl` back-compat pass).

## End-to-End Tests
- **Status:** possible
- `verify_github_actions_ci_on_dev`: push `dev` and observe GitHub Actions CI (`lint`, `test` ubuntu + windows, `native`, `python`, `webui` native + wasm32) conclude `success` via `gh run list` / `gh run watch`, plus `check-substrate.sh` (`substrate-complete`) and `check-book.sh` (`valid v2 Book`).
