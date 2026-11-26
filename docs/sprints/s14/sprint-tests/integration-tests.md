# Sprint 14 Integration Test Results

## Scope
Verified full root Cargo workspace build, lint, formatting, and engine/store/tessellation integration regression suite after `T-004` (`95675d6`).

## Commands Executed
- `cargo fmt --all -- --check` → exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0 (0 warnings)
- `cargo test --workspace` → exit 0 (**331 passed**, 0 failed, 0 ignored)

## Breakdown by Crate
- `carbide-core`: 8 passed (DAG + workbook/sheet serde)
- `carbide-formula`: 174 unit + 53 integration (`dag_recompute`: 3, `hex_conway`: 3, `persistence`: 3, `reference_examples`: 35, `scale`: 4, `transpile_differential`: 1, `transpile_fuzz`: 4) = 227 passed
- `carbide-store`: 9 passed (`.crbd` v2 archive round-trip, `FORMAT_VERSION = 2` unchanged)
- `carbide-tess`: 77 unit + 10 integration (`lattice_geometry`) = 87 passed
- Combined total across root workspace + UI crate: **646/646 tests passing**.
