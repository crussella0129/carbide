# Sprint 15 Integration Test Results

## Scope
Verified target-filtered MSRV alignment (`verify_msrv_alignment`), Dependabot configuration (`verify_dependabot_config`), and cross-crate engine/store/tessellation integration suites after `T-004`.

## MSRV Alignment (`verify_msrv_alignment`)
Executed target-filtered `cargo metadata --locked --format-version 1 --filter-platform <triple>` with normalized `(major, minor, patch)` tuple comparison and `edition == "2024"` → `(1, 85, 0)` floor:
- **Root workspace (`Cargo.toml`, `--all-features`, `x86_64-unknown-linux-gnu` + `x86_64-pc-windows-msvc`):**
  - Resolved max: **`1.88`** (driven by `libloading 0.9.1`, `rust_version = "1.88.0"`)
  - Declared `[workspace.package].rust-version`: **`"1.88"`** → exact match
- **UI crate (`apps/carbide-ui/Cargo.toml`, `x86_64-unknown-linux-gnu` + `x86_64-pc-windows-msvc` + `wasm32-unknown-unknown`):**
  - Resolved max: **`1.95`** (driven by `egui`/`eframe`/`ecolor` `0.37.0`, `rust_version = "1.95"`, `edition = "2024"`)
  - Declared `[package].rust-version`: **`"1.95"`** → exact match

## Dependabot Configuration (`verify_dependabot_config`)
Verified `.github/dependabot.yml` (`version: 2`) declares weekly `target-branch: "dev"` entries for:
1. `package-ecosystem: "github-actions"`, `directory: "/"`
2. `package-ecosystem: "cargo"`, `directory: "/"`
3. `package-ecosystem: "cargo"`, `directory: "/apps/carbide-ui"`

## Workspace Integration Breakdown
- `carbide-core`: 8 passed
- `carbide-formula`: 174 unit + 53 default integration (`dag_recompute`: 3, `hex_conway`: 3, `persistence`: 3, `reference_examples`: 35, `scale`: 4, `transpile_differential`: 1, `transpile_fuzz`: 4) + 14 `--features python` (`11` unit + `3` `cross_engine`) + 4 `--features native` (`2` unit + `2` `native_differential`)
- `carbide-store`: 9 passed (`.crbd` v2 `FORMAT_VERSION = 2` unchanged)
- `carbide-tess`: 77 unit + 10 integration (`lattice_geometry`)
- `carbide-ui`: 315 unit passed
