# Sprint 14 Test Plan

## Intent Traceability
| Intent | Acceptance criterion | Build task / EARS clause | Verification |
|--------|----------------------|--------------------------|--------------|
| [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) | AC-1: Voronoi supports formatting (`voronoi_formats`), notes (`voronoi_notes`), and copy/cut/paste without breaking older `.crbd` v2 snapshots (`#[serde(default)]`) | T-001, T-002, T-003 / WHEN `UiSnapshot` round-trips `voronoi_formats` + `voronoi_notes` AND legacy v2 JSON omits them AND Voronoi copy/cut/paste runs THEN round-trip is lossless, strict legacy decode defaults empty, and same/cross-lattice paste behaves per spec | `verify_ui_snapshot_roundtrip_and_compat`, `verify_voronoi_clipboard_and_paste` |
| [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) | AC-2: Multi-cell Voronoi marquee selections (`Selection.extra`) operate through format, clear, widget toggle, and copy pipelines | T-001, T-003, T-004 / WHEN Voronoi `Selection.extra` holds non-contiguous marquee cells THEN format/toggle, clear, widget toggle/button, and copy/cut operate on every cell in `selection.cells()` | `verify_voronoi_marquee_operations`, `verify_unified_format_and_widget_helpers` |
| [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md) | AC-3: Shared helpers/traits eliminate copy-pasted dispatch arms across square, hex, triangle, and Voronoi where semantics agree, covered by unit tests | T-001, T-002, T-003, T-004 / WHEN unified `FormatEdit`/`merge_format_edits`, `dedup_sheet_targets`, `toggle_widget_kind`, and `open_note` run across all 4 lattices THEN unit tests pass with zero warnings, zero `#[allow(...)]`, and every `app/*.rs` ≤ 1,000 lines | `verify_unified_format_and_widget_helpers`, `verify_module_hygiene_and_wasm32` |

## Execution Order Guarantee
All verification gates below run in full after `T-004` completes (in addition to incremental `fmt`/`clippy`/`test` checks during `T-001`–`T-004`).

## Unit Tests
### Formatting, history coalescing, and `UiSnapshot` persistence (T-001, T-002, T-004)
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- `verify_unified_format_and_widget_helpers`: unit tests in `apps/carbide-ui/src/app/tests.rs` and `apps/carbide-ui/src/app/helpers.rs` covering unified `merge_format_edits` across `CellId::{Square, Hex, Triangle, Voronoi}`, `dedup_sheet_targets` (atomic multi-sheet cut/paste deduplication), and generic discriminant-safe `toggle_widget_kind` across all 4 lattices (all-off → all-on, mixed → all-on, all-on → all-off, including non-contiguous Voronoi marquee cells).
- `verify_ui_snapshot_roundtrip_and_compat`: unit tests in `apps/carbide-ui/src/state_io.rs` verifying `UiSnapshot` round-trips populated `voronoi_formats` and `voronoi_notes` through `snapshot_to_ui_state`/`ui_state_to_snapshot`, AND decoding a legacy v2 JSON object lacking `voronoi_formats` and `voronoi_notes` directly via `serde_json::from_value::<UiSnapshot>(...).unwrap()` (C-004) succeeds with empty Voronoi maps while preserving non-default legacy fields (`active_sheet`, `stage_mode`, `square_notes`).

### Voronoi clipboard, cut/paste, and marquee operations (T-003, T-004)
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- `verify_voronoi_clipboard_and_paste`: unit tests in `apps/carbide-ui/src/clipboard.rs` (`SourceLattice::Voronoi`, `from_voronoi`, same-lattice formula preservation, cross-lattice value degradation, `ValuesOnly`, single `Option<CutOrigin>` with `mark_as_voronoi_cut`/`cut_voronoi_cells`/`has_cut`/`consume_cut`) and `apps/carbide-ui/src/app/voronoi_helpers.rs` (`voronoi_paste_targets` covering sequential placement from cursor, out-of-bounds seed truncation, post-paste selection coords, and cross-lattice degradation).
- `verify_voronoi_marquee_operations`: unit tests verifying `Selection<VoronoiCoord>::cells()` deduplication, marquee fan-out for format targets, clear targets, copy/cut cells, and `toggle_widget_kind`.

### Hygiene, line bounds, and wasm32 filter parity (T-001..T-004)
- **Intent:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- `verify_module_hygiene_and_wasm32`:
  - `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`
  - `cargo test --manifest-path apps/carbide-ui/Cargo.toml`
  - `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown`
  - Verify `max(lines) <= 1000` for every `*.rs` under `apps/carbide-ui/src/app/`, zero `#!?[allow(` attributes under `apps/carbide-ui/src/app/`, and wasm32 filters in `apps/carbide-ui/src/app/io.rs` use `&["crbd"]` (save) and `&["crbd", "tscl"]` (open).

## Integration Tests
### Full root workspace & `.crbd` store regression
- **Intents:** [INT-0003](../../../intents/INT-0003-lattice-ui-parity-and-unification.md)
- `verify_workspace_regression`: run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` to confirm all 331+ workspace tests (including `carbide-store` v2 round-trip tests and `carbide-formula` Voronoi engine tests) remain green.

## End-to-End Tests
- **Status:** possible
- `verify_github_actions_ci_on_dev`: push `dev` and verify GitHub Actions CI (`lint`, `test` ubuntu + windows, `native`, `python`, `webui` native + wasm32) concludes `success` via `gh run watch`, plus `check-substrate.sh` (`substrate-complete`) and `check-book.sh` (`valid v2 Book`).
