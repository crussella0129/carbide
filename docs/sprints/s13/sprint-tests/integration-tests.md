# Sprint 13 Integration Tests Evidence

- **Toolchain:** `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`
- **Verified against:** root workspace crates alongside Sprint 13 `apps/carbide-ui/src/app/` modularization

## `verify_workspace_regression` (INT-0002 AC-2, AC-3)
- `cargo fmt --all -- --check` — **PASS** (exit 0)
- `cargo clippy --workspace --all-targets -- -D warnings` — **PASS** (exit 0, zero warnings)
- `cargo test --workspace --quiet` — **PASS** (331 passed / 0 failed / 0 ignored):
  - `carbide-core`: 8 unit tests passed
  - `carbide-formula`: 174 unit + 53 integration tests (`dag_recompute` 3, `hex_conway` 3, `persistence` 3, `reference_examples` 35, `scale` 4, `transpile_differential` 1, `transpile_fuzz` 4) passed
  - `carbide-store`: 9 unit tests passed
  - `carbide-tess`: 77 unit + 10 `lattice_geometry` integration tests passed
