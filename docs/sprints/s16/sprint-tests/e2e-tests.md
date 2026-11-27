# Sprint 16 End-to-End Tests

## Scope
- `verify_msrv_and_github_ci`: target-filtered MSRV alignment (`root = 1.89`, `ui = 1.95`), GitHub Actions CI run across all 6 jobs on `dev`, plus Book v2 and Sprint Loops substrate validation.

## Execution & Results
1. **Target-filtered MSRV verification (`cargo metadata --filter-platform`):**
   - Root workspace (`Cargo.toml`, `x86_64-unknown-linux-gnu` + `x86_64-pc-windows-msvc`, `--all-features`): max MSRV = **`1.89`** (`Cargo.toml` `[workspace.package].rust-version = "1.89"`, matching `zip 9.8.2` deduplicated across `carbide-store` and `calamine 0.38.0`).
   - UI workspace (`apps/carbide-ui/Cargo.toml`, `x86_64-unknown-linux-gnu` + `x86_64-pc-windows-msvc` + `wasm32-unknown-unknown`): max MSRV = **`1.95`** (`apps/carbide-ui/Cargo.toml` `[package].rust-version = "1.95"`, driven by `egui`/`eframe 0.37.0`).
2. **GitHub Actions CI on `dev` (`43ef6423e91a7f1076f2e341fb4e1e140eacf3f2`):**
   - Run [#45949651858](https://github.com/crussella0129/carbide/actions/runs/45949651858) — overall conclusion: **`success`** (**6/6 jobs green**):
     - `rustfmt + clippy`: `success`
     - `build + test (ubuntu-latest)`: `success`
     - `build + test (windows-latest)`: `success`
     - `native compile pipeline`: `success`
     - `python engine`: `success`
     - `web front-end (wasm + native)`: `success`
3. **Book v2 & Sprint Loops Substrate Validation:**
   - `bash .../check-book.sh` → `check-book: valid v2 Book (6 intent chapters)` (exit 0)
   - `bash .../check-substrate.sh` → `substrate-complete` (exit 0)
