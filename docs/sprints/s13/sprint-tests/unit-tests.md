# Sprint 13 Unit Tests Evidence

- **Toolchain:** `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`, targets `x86_64-pc-windows-msvc` + `wasm32-unknown-unknown`
- **Verified against commit:** `d528699` (post-`T-004`; merged cleanly with Dependabot `.github/workflows/ci.yml` updates at `2c65eaa6418e055a9d197ff5237e8b64793b05b3` with zero Rust source changes)

## `verify_module_line_bounds` (INT-0002 AC-1)
- `apps/carbide-ui/src/app.rs` removed (replaced by `apps/carbide-ui/src/app/` directory).
- Measured line counts across all 15 `.rs` files under `apps/carbide-ui/src/app/` (`max = 839 <= 1000` — **PASS**):
  - `apps/carbide-ui/src/app/tests.rs`: 839 lines
  - `apps/carbide-ui/src/app/render_square.rs`: 761 lines
  - `apps/carbide-ui/src/app/edit.rs`: 688 lines
  - `apps/carbide-ui/src/app/mod.rs`: 664 lines
  - `apps/carbide-ui/src/app/commands.rs`: 658 lines
  - `apps/carbide-ui/src/app/cells.rs`: 612 lines
  - `apps/carbide-ui/src/app/helpers.rs`: 513 lines
  - `apps/carbide-ui/src/app/render_hex.rs`: 482 lines
  - `apps/carbide-ui/src/app/dialogs.rs`: 471 lines
  - `apps/carbide-ui/src/app/render_voronoi.rs`: 427 lines
  - `apps/carbide-ui/src/app/render_triangle.rs`: 384 lines
  - `apps/carbide-ui/src/app/demos.rs`: 338 lines
  - `apps/carbide-ui/src/app/types.rs`: 312 lines
  - `apps/carbide-ui/src/app/io.rs`: 307 lines
  - `apps/carbide-ui/src/app/voronoi_helpers.rs`: 234 lines

## `verify_public_api_unchanged` (INT-0002 AC-2)
- `git diff main -- apps/carbide-ui/src/lib.rs apps/carbide-ui/src/main.rs` — **PASS** (empty diff)
- `git grep -n "^[[:space:]]*pub " apps/carbide-ui/src/app/` — **PASS** (matches only `pub struct CarbideApp` at `mod.rs:143` and `pub fn new` at `demos.rs:24`)
- `git grep -n "pub(crate) fn \(capture_state\|restore_state\)" apps/carbide-ui/src/app/io.rs` — **PASS** (matches `io.rs:212` and `io.rs:274`)

## `verify_ui_fmt_clippy_no_allows` (INT-0002 AC-3)
- `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check` — **PASS** (exit 0)
- `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` — **PASS** (exit 0, zero warnings)
- `git grep --untracked -nE "#!?\[[[:space:]]*allow\(" apps/carbide-ui/src/app/` — **PASS** (0 matches)

## `verify_ui_tests_and_wasm32` (INT-0002 AC-2)
- `cargo test --manifest-path apps/carbide-ui/Cargo.toml --quiet` — **PASS** (308 passed / 0 failed / 0 ignored, zero warnings)
- `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` — **PASS** (exit 0, zero warnings)
