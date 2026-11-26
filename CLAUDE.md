# CLAUDE.md — Carbide

Project-specific guidance for contributors and coding agents working in Carbide.

## What this project is

Carbide is a pure-Rust, DAG-evaluated spreadsheet with **non-square tessellating cells** (squares, hexagons, triangles, and Voronoi cells — with further tilings on the roadmap) and a **switchable formula language per cell** (Carbide / Excel-lite with an optional Carbide→Rust native transpiler, plus embedded Python via PyO3).

- **Project Book (`docs/`):** Canonical Sprint Loops Book v2 state (`docs/intents/`, `docs/work/`, `docs/sprints/`). Historical architectural decisions (ADR-001 through ADR-015) live in `docs/history/decisions-legacy.md`.
- **Architecture & specs:** See `PLAN.md`, `docs/all-rust-roadmap.md`, and `docs/carbide/README.md`.

## Languages and stack

This is a **100% Rust** repository split across two Cargo workspaces:

- **Core workspace** (root `Cargo.toml`, crates under `crates/`):
  - `carbide-core`: `Workbook`, `Sheet`, `Cell`, `CellValue`, `Dag`, `SheetExtent`.
  - `carbide-tess`: `Lattice` trait, `LatticeHandle`, and concrete tessellations (`square`, `hex`, `triangle`, `voronoi`).
  - `carbide-formula`: `WorkbookEngine`, `FormulaEngine` trait, `excellite` (Carbide interpreter + stdlib), `transpile` (Carbide→Rust codegen + `--features native` `cdylib` pipeline), and `--features python` (PyO3).
  - `carbide-store`: `.crbd` v2 zip archive persistence (opaque `ui.json` sidecar + `.tscl` load back-compat).
  - `carbide-cli`: structural CLI entry point for future headless commands.
- **UI sub-workspace** (`apps/carbide-ui/Cargo.toml`):
  - Pure-Rust `egui` / `eframe` application compiled to both native desktop and `wasm32-unknown-unknown`, consuming the core crates directly in-process. Keep domain logic in `crates/`, not in the UI layer.

Formula engine Cargo features in `crates/carbide-formula/Cargo.toml`: `native` (Carbide→Rust `cdylib` compilation via `libloading`), `python` (PyO3), plus reserved `rhai` and `rustnative` flags so default builds stay lean without optional toolchains.

## Working norms

- **Workspace-aware tests.** Run `cargo test -p <crate>` for the crate you changed, or `cargo test --workspace` when changes cross crate boundaries.
- **Formatters/linters:** `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` (plus the same with `--manifest-path apps/carbide-ui/Cargo.toml` for UI changes).
- **No premature abstraction.** Build what the current sprint intent requires; avoid speculative traits or unused lattices.
- **The `Lattice` trait is load-bearing.** Changes to `carbide-tess::Lattice` ripple into every UI renderer and formula engine. Review carefully before altering its signature.

## What NOT to do without asking

- **Don't add an unrequested formula engine** (e.g., WASM-based Python or an LLM-based engine) without an explicit ask.
- **Don't change the `.crbd` zip schema** without bumping `FORMAT_VERSION` in `crates/carbide-store/src/lib.rs` and adding a backward-compatible upgrade path (additive `#[serde(default)]` fields inside `ui.json` follow ADR-001; see `docs/history/decisions-legacy.md`).
- **Don't merge a partial new lattice** without a `Lattice` impl, address parser, UI renderer, and test coverage.

## Build and test commands

```bash
# Core workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p carbide-formula --features native --all-targets -- -D warnings
cargo run --bin carbide-cli

# Pure-Rust UI (native + wasm32)
cargo run --manifest-path apps/carbide-ui/Cargo.toml
cargo test --manifest-path apps/carbide-ui/Cargo.toml
cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings
cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown
```

## Running shell commands

The shell starts in the repo root (`C:\Users\charl\carbide`) — **do not prepend `cd`** to commands. Combining `cd` with output redirection (`>`, `2>&1`) in a compound command trips sandbox path-resolution guards.

- `git`, `gh`, and any repo-root command: run bare — no `cd` prefix.
- `cargo` for `apps/carbide-ui` (its own `[workspace]`): pass `--manifest-path apps/carbide-ui/Cargo.toml` — e.g. `cargo test --manifest-path apps/carbide-ui/Cargo.toml` — rather than changing directories.
- Watching a CI run to completion (`gh run watch --exit-status`) is a routine step of the loop; `.claude/settings.json` allowlists read-only `gh run` commands.

## Repo identity

- Owner: `crussella0129`
- GitHub: `https://github.com/crussella0129/carbide` (`git@github.com:crussella0129/carbide.git`)
- License: MIT OR Apache-2.0 (dual)
- Branches: `main` (base) / `dev` (Sprint Loops work branch, see `docs/work/remote-profile.md`)
