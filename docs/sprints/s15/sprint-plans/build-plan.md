Finalized - DO NOT EDIT

# Sprint 15 Build Plan

## Intents
- [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md) — state: `planned`; acceptance criteria covered: AC-1 (every intentional major dependency bump verified across all 6 CI jobs: lint, Ubuntu + Windows workspace tests, `native`, `python`, and `webui` native + wasm32), AC-2 (`Cargo.toml` and `apps/carbide-ui/Cargo.toml` `rust-version` match the actual minimum toolchain required by each resolved dependency tree across supported targets), AC-3 (`.github/dependabot.yml` covers `github-actions` plus both Cargo manifests `/` and `/apps/carbide-ui` targeting `dev`).

## Schema Tree
- Sprint 15: Dependency, toolchain, and CI modernization (`INT-0004`)
  - Root workspace core dependencies
    - T-001: Upgrade root workspace `hashbrown` (`0.14` → `0.17`), `glam` (`0.27` → `0.34`), and `thiserror` (`1` → `2`)
  - Feature-gated formula engine dependencies
    - T-002: Upgrade `pyo3` (`0.23` → `0.30`) and `libloading` (`0.8` → `0.9`) in `carbide-formula` and verify `--features python` and `--features native`
  - UI workspace dependencies & ADR-003 cleanup
    - T-003: Upgrade `rfd` (`0.14` → `0.18`, dropping the ADR-003 wasm32 `gtk3` workaround) and `egui`/`eframe` (`0.29` → `0.37`, adopting upstream `eframe 0.37` defaults) in `apps/carbide-ui`
  - Accurate MSRV alignment & Dependabot expansion
    - T-004: Set exact target-filtered `rust-version` in `Cargo.toml` and `apps/carbide-ui/Cargo.toml` from resolved `cargo metadata`, and expand `.github/dependabot.yml` to cover both Cargo manifests targeting `dev`

## Scope & Invariants (Incorporating Plan Critic C-001..C-005)
- **Intentional exclusions:** `zip` stays at `2` in `crates/carbide-store/Cargo.toml` (deflate-only v2 is stable, MSRV 1.73, and jumping 7 major versions `2` → `9` adds unnecessary `.crbd` I/O risk; Dependabot will surface future updates on `dev`). `.crbd` `FORMAT_VERSION` remains `2`.
- **`eframe 0.37` backend decision (C-001):** Intentionally adopt `eframe = "0.37"` upstream defaults (`wgpu` + `system_fonts` + `clipboard` + `links` + `winit/default` + `accesskit` + `wayland` + `x11` + `web_screen_reader`). `egui-wgpu/default` retains `wgpu/webgl` fallback on wasm32, `fontique/fontconfig-dlopen` and `winit/default` (`wayland-backend/dlopen`) unify cleanly with `rfd = "0.18"` so Ubuntu CI apt dependencies stay unchanged.
- **ADR-003 superseding (C-004):** `rfd 0.18`'s `build.rs` handles wasm targets directly (`_ if is_wasm => {}`), eliminating `features = ["gtk3"]` on `wasm32`. Update stale `rfd` comments in `apps/carbide-ui/Cargo.toml` and record ADR-003 as superseded in `docs/history/decisions-legacy.md`.
- **Module & lint hygiene (`INT-0002` / C-002):** Every `.rs` file under `apps/carbide-ui/src/app/` stays ≤ 1,000 lines, zero `#![allow(...)]`/`#[allow(...)]` attributes under `apps/carbide-ui/src/app/`, and `-D warnings` passes across both native AND `wasm32-unknown-unknown` (`cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings`).

## Execution Sequence

### T-001: Upgrade root workspace `hashbrown` (`0.17`), `glam` (`0.34`), and `thiserror` (`2`)
- **Intent:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- **Touches:** `Cargo.toml`, `Cargo.lock`, `apps/carbide-ui/Cargo.lock` (plus minimal call-site adaptations in `crates/` if any deprecation/API change surfaces)
- **Depends on:** (none)
- **Acceptance criterion:** AC-1 (verified across root workspace and UI crate builds/tests).
- **Success criterion (EARS):**
  - **WHEN** `[workspace.dependencies]` updates `hashbrown = "0.17"`, `glam = "0.34"`, and `thiserror = "2"` and lockfiles refresh, **THEN** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (331+ tests), and `cargo test --manifest-path apps/carbide-ui/Cargo.toml` (315+ tests) **SHALL** all exit 0 with zero warnings, and `Cargo.lock` **SHALL** contain only one `hashbrown` major line (`0.17.x`).
- **Notes:** Preserves `hashbrown` and `glam` `serde` features (`features = ["serde"]`).

### T-002: Upgrade `pyo3` (`0.30`) and `libloading` (`0.9`) in `carbide-formula`
- **Intent:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- **Touches:** `crates/carbide-formula/Cargo.toml`, `crates/carbide-formula/src/python/mod.rs`, `crates/carbide-formula/src/transpile/native.rs`, `Cargo.lock`
- **Depends on:** T-001
- **Acceptance criterion:** AC-1 (verified across `--features python` and `--features native` clippy + test targets).
- **Success criterion (EARS):**
  - **WHEN** `pyo3` is bumped to `"0.30"` (`features = ["auto-initialize"]`) and `python/mod.rs` migrates `Python::with_gil` → `Python::attach` (and any other 0.30 API updates), **THEN** `cargo clippy -p carbide-formula --features python --all-targets -- -D warnings` and `cargo test -p carbide-formula --features python` **SHALL** exit 0 with all Python engine tests passing on Python 3.13.
  - **WHEN** `libloading` is bumped to `"0.9"` and `transpile/native.rs` is adapted as needed, **THEN** `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings` and `cargo test -p carbide-formula --features native` **SHALL** exit 0 with all native-compile pipeline tests passing.
- **Notes:** `apps/carbide-ui/Cargo.lock` refresh stays in `T-003` (C-005). Verify default-feature `cargo test --workspace` still passes without requiring Python or `libloading`.

### T-003: Upgrade `rfd` (`0.18`, dropping ADR-003 wasm `gtk3` workaround) and `egui`/`eframe` (`0.37`) in `apps/carbide-ui`
- **Intent:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- **Touches:** `apps/carbide-ui/Cargo.toml`, `apps/carbide-ui/Cargo.lock`, `apps/carbide-ui/src/*.rs`, `apps/carbide-ui/src/app/*.rs`, `docs/history/decisions-legacy.md` (ADR-003 superseded note; `.github/workflows/ci.yml` only if Linux CI requires an extra system library)
- **Depends on:** T-002
- **Acceptance criterion:** AC-1 (verified across native `fmt`, `clippy --all-targets -- -D warnings`, `test`, and `wasm32-unknown-unknown` clippy `-D warnings` + build).
- **Success criterion (EARS):**
  - **WHEN** `apps/carbide-ui/Cargo.toml` upgrades `rfd` to `"0.18"` on native and `{ version = "0.18", default-features = false }` on `wasm32` (removing `features = ["gtk3"]` and updating the stale `rfd` comments + ADR-003 status, C-004), AND upgrades `egui` and `eframe` to `"0.37"` (adopting upstream `eframe 0.37` defaults per C-001) with all UI call sites migrated cleanly (`eframe::App::ui`, `egui::Panel::top`/`bottom` + `CentralPanel::show_inside`, `ui.close()`, `Margin::symmetric(i8, i8)`, `CornerRadius`/`StrokeKind`, tooltips/popups, C-002), **THEN** `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`, `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path apps/carbide-ui/Cargo.toml` (all 315+ tests), `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown --all-targets -- -D warnings`, and `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` **SHALL** all exit 0 with zero warnings.
  - **WHEN** line counts and allow attributes under `apps/carbide-ui/src/app/` are checked after migration, **THEN** every `.rs` file **SHALL** be ≤ 1,000 lines and zero `#![allow(...)]`/`#[allow(...)]` attributes **SHALL** be present.
- **Notes:** Preserve `.crbd` v2 snapshot compatibility (`format.rs` `color_rgba_opt` `to_srgba_unmultiplied`/`from_rgba_unmultiplied` and all `state_io`/`format` tests pass unchanged). **Rollback boundary (C-005):** `T-001`–`T-002` and `rfd 0.18` stand independently if an unexpected `egui`/`eframe 0.37` blocker arises.

### T-004: Align workspace & UI `rust-version` MSRVs and expand `.github/dependabot.yml`
- **Intent:** [INT-0004](../../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- **Touches:** `Cargo.toml`, `apps/carbide-ui/Cargo.toml`, `.github/dependabot.yml`
- **Depends on:** T-003
- **Acceptance criterion:** AC-2 (`Cargo.toml` and `apps/carbide-ui/Cargo.toml` `rust-version` match actual dependency-tree MSRVs across supported targets), AC-3 (`.github/dependabot.yml` covers `github-actions` plus both Cargo manifests `/` and `/apps/carbide-ui` targeting `dev`).
- **Success criterion (EARS):**
  - **WHEN** `cargo metadata --locked --format-version 1 --all-features --filter-platform <triple>` (for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc` on root; plus `wasm32-unknown-unknown` on `apps/carbide-ui`) computes the maximum normalized `(major, minor, patch)` version across all resolved dependencies (treating `edition == "2024"` with `rust_version: null` as `(1, 85, 0)`, C-003), **THEN** `Cargo.toml` `[workspace.package].rust-version` and `apps/carbide-ui/Cargo.toml` `[package].rust-version` **SHALL** each match their exact canonical `X.Y` (or `X.Y.Z` when patch > 0) resolved maximum.
  - **WHEN** `.github/dependabot.yml` is parsed, **THEN** it **SHALL** define valid Dependabot v2 weekly update entries targeting `dev` for `github-actions` (`/`), `cargo` (`/`), and `cargo` (`/apps/carbide-ui`).
- **Notes:** Validate `.github/dependabot.yml` syntax and confirm all 6 GitHub Actions CI jobs pass on `dev`.
