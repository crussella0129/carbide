# Sprint 13 Test Plan

## Intent Traceability
| Intent | Acceptance criterion | Build task / EARS clause | Verification |
|--------|----------------------|--------------------------|--------------|
| [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) | AC-1: No single file under `apps/carbide-ui/src/app/` exceeds 1,000 lines | T-001, T-002, T-003, T-004 / WHEN line counts of every `.rs` file under `apps/carbide-ui/src/app/` are measured THEN every file ≤ 1,000 lines | `verify_module_line_bounds` |
| [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) | AC-2: Public crate API (`carbide_ui::CarbideApp`), `pub(crate)` snapshot API, and all 308 UI + 331 workspace tests unchanged and green across native and `wasm32-unknown-unknown` | T-001..T-004 / WHEN `lib.rs` is unchanged, `capture_state`/`restore_state` stay `pub(crate)`, AND UI tests + wasm32 build + root workspace tests + GitHub CI run THEN exit 0 with 639/639 total tests passing | `verify_public_api_unchanged`, `verify_ui_tests_and_wasm32`, `verify_workspace_regression`, `verify_github_actions_ci_on_dev` |
| [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md) | AC-3: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with zero new `#[allow(...)]` suppressions | T-001..T-004 / WHEN `fmt`, `clippy -D warnings`, and allow-attribute grep run under `apps/carbide-ui/src/app/` (plus CI lint/webui jobs) THEN exit 0 with zero warnings and zero allow attributes | `verify_ui_fmt_clippy_no_allows`, `verify_workspace_regression`, `verify_github_actions_ci_on_dev` |

## Execution Order Guarantee
All verification gates below run in full after `T-004` completes (in addition to incremental `fmt`/`clippy`/`test` checks during `T-001`–`T-004`).

## Unit Tests
### Module size & hygiene checks (T-001..T-004)
- **Intent:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- `verify_module_line_bounds`: measure line count of every `*.rs` file under `apps/carbide-ui/src/app/` and verify `max(lines) <= 1000` and `apps/carbide-ui/src/app.rs` no longer exists.
- `verify_public_api_unchanged`: verify `git diff main -- apps/carbide-ui/src/lib.rs apps/carbide-ui/src/main.rs` is empty, `git grep -n "^[[:space:]]*pub " apps/carbide-ui/src/app/` matches only `pub struct CarbideApp` and `pub fn new`, and `git grep -n "pub(crate) fn \(capture_state\|restore_state\)" apps/carbide-ui/src/app/io.rs` matches both methods.
- `verify_ui_fmt_clippy_no_allows`: run `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`, `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`, and `git grep --untracked -nE "#!?\[[[:space:]]*allow\(" apps/carbide-ui/src/app/` (must return 0 matches).

### UI unit test suite + wasm32 target (T-001..T-004)
- **Intent:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- `verify_ui_tests_and_wasm32`: run `cargo test --manifest-path apps/carbide-ui/Cargo.toml` (all 308 tests pass, 0 warnings) and `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` (exit 0, 0 warnings).

## Integration Tests
### Workspace regression check
- **Intents:** [INT-0002](../../../intents/INT-0002-modularize-carbide-ui-app.md)
- `verify_workspace_regression`: run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (331/331 workspace tests pass) to confirm zero cross-workspace breakage.

## End-to-End Tests
- **Status:** possible
- `verify_github_actions_ci_on_dev`: push `dev` and confirm GitHub Actions CI (`lint`, `test` ubuntu + windows, `native`, `python`, `webui` native + wasm32) concludes `success` via `gh run watch`, plus `check-substrate.sh` (`substrate-complete`) and `check-book.sh` (`valid v2 Book`).
