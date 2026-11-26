# Sprint 12 Unit Tests Evidence

- **Toolchain:** `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`, targets `x86_64-pc-windows-msvc` + `wasm32-unknown-unknown`
- **Verified against commit:** `70e25fcfcf7d7ecafe495ab173e436de8ff9ba32` (post-`T-004`, satisfying Plan Critic C-003)

## `verify_ui_fmt_clippy_test_wasm` (INT-0001 AC-2, AC-6)
- `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check` — **PASS** (exit 0)
- `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings` — **PASS** (exit 0, zero warnings; all 21 prior `float_literal_f32_fallback` errors resolved)
- `cargo test --manifest-path apps/carbide-ui/Cargo.toml` — **PASS** (308 passed / 0 failed / 0 ignored, zero compiler warnings)
- `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` — **PASS** (exit 0, zero warnings)

## `verify_no_dead_code_allow_in_app` (INT-0001 AC-3)
- `git grep -n "allow(dead_code)" apps/carbide-ui/src/app.rs` — **PASS** (0 matches)

## `verify_adr015_repo_urls` (INT-0001 AC-4, AC-6)
- `git grep -i "crussella0129/tescellate" -- Cargo.toml CLAUDE.md apps/carbide-ui/src/app.rs` — **PASS** (0 matches)
- `git grep -n "crussella0129/carbide" -- Cargo.toml CLAUDE.md apps/carbide-ui/src/app.rs` — **PASS** (3 matches: `Cargo.toml:17`, `CLAUDE.md:68`, `apps/carbide-ui/src/app.rs:2175`)
- `git grep -n "becomes" crates/carbide-cli/src/main.rs` — **PASS** (0 matches)
- `git grep -n "squares, hexagons, triangles, Voronoi" apps/carbide-ui/src/app.rs` — **PASS** (matches line 2167)

## `verify_ci_push_branches` (INT-0001 AC-5)
- `git grep -n "branches: \[main, dev\]" .github/workflows/ci.yml` — **PASS** (matches line 5; all 5 jobs `lint`, `test`, `native`, `python`, `webui` preserved intact)

## `verify_docs_and_metadata_accuracy` (INT-0001 AC-6)
- `git grep -nE "Status: Phase 0|Nothing user-visible works yet|apps/desktop|Node\.js 20|JSON-RPC IPC|Tauri|prettier|eslint|npm install|npm run dev" -- README.md CLAUDE.md` — **PASS** (0 matches)
- `git grep -n "docs/history/decisions-legacy.md" CLAUDE.md` — **PASS** (2 matches)
- `git grep -n "native" CLAUDE.md` — **PASS** (matches `native` Cargo feature and clippy command)
- `cargo run -p carbide-cli` — **PASS** (exit 0; caught and fixed stale `cargo run --bin carbide-cli` typo in `CLAUDE.md` before final verification)
