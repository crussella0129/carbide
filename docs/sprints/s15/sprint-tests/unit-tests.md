# Sprint 15 Unit Test Results

## Scope
Verified all unit-level assertions from [test-plan.md](../sprint-plans/test-plan.md) after `T-004` and the MSRV-1.88 Clippy lint cleanup commit completed.

## Commands Executed
- **Root workspace + feature flags:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo clippy --workspace --all-targets -- -D warnings` → exit 0 (0 warnings)
  - `cargo test --workspace --quiet` → exit 0 (**331 passed**, 0 failed)
  - `cargo clippy -p carbide-formula --features python --all-targets -- -D warnings` → exit 0 (0 warnings)
  - `cargo test -p carbide-formula --features python --quiet` → exit 0 (**185** unit + **56** integration tests passed on `pyo3 0.30.1` + Python `3.13.11`)
  - `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings` → exit 0 (0 warnings)
  - `cargo test -p carbide-formula --features native --quiet` → exit 0 (**176** unit + **55** integration tests passed on `libloading 0.9.1`)
  - `grep -A 1 '^name = "hashbrown"$' Cargo.lock` → single line: `hashbrown 0.17.1`
- **UI crate (native + wasm32) & hygiene:**
  - `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check` → exit 0
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` → exit 0 (0 warnings)
  - `cargo test --manifest-path apps/carbide-ui/Cargo.toml --quiet` → exit 0 (**315 passed**, 0 failed — including `state_io` and `format` `egui::Color32` serde compatibility on `egui 0.37.0`)
  - `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown --all-targets -- -D warnings` → exit 0 (0 warnings on all `#[cfg(target_arch = "wasm32")]` paths)
  - `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` → exit 0 (0 warnings; `apps/carbide-ui/Cargo.toml` uses `rfd = { version = "0.18", default-features = false }` without `gtk3`, superseding ADR-003)
  - `wc -l apps/carbide-ui/src/app/*.rs | sort -n` → max **819 lines** (`tests.rs`), all 15 files ≤ 1,000 lines
  - `! git grep --untracked -nE '#!?\[[[:space:]]*allow\(' apps/carbide-ui/src/app/` → 0 matches

## MSRV-1.88 Lint Cleanup
Bumping `Cargo.toml` `[workspace.package].rust-version` from `1.75` to `1.88` enabled MSRV-gated Clippy lints across the root workspace, resolved in 3 sites:
- `crates/carbide-tess/src/voronoi.rs:109` (`chunks_exact(3)` → `as_chunks::<3>().0`)
- `crates/carbide-formula/src/excellite/funcs/logical.rs:58` (`args.len() % 2 != 0` → `!args.len().is_multiple_of(2)`)
- `crates/carbide-formula/tests/reference_examples.rs:413, 419` (`repeat("+1").take(n)` → `repeat_n("+1", n)`)
