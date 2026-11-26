# Sprint 12 Integration Tests Evidence

- **Toolchain:** `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`
- **Verified against commit:** `70e25fcfcf7d7ecafe495ab173e436de8ff9ba32` (post-`T-004`)

## `verify_workspace_fmt_clippy_test` (INT-0001 AC-1, AC-6)
- `cargo fmt --all -- --check` — **PASS** (exit 0)
- `cargo clippy --workspace --all-targets -- -D warnings` — **PASS** (exit 0, zero warnings)
- `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings` — **PASS** (exit 0, zero warnings)
- `cargo test --workspace` — **PASS** (331 passed / 0 failed / 0 ignored):
  - `carbide-core` lib unit tests: 8 passed
  - `carbide-formula` lib unit tests: 174 passed
  - `carbide-formula` integration tests:
    - `dag_recompute`: 3 passed
    - `hex_conway`: 3 passed
    - `persistence`: 3 passed
    - `reference_examples`: 35 passed
    - `scale`: 4 passed
    - `transpile_differential`: 1 passed
    - `transpile_fuzz`: 4 passed
  - `carbide-store` lib unit tests: 9 passed (including `.tscl` v2 back-compat)
  - `carbide-tess` lib unit tests: 77 passed
  - `carbide-tess` `lattice_geometry` integration tests: 10 passed
