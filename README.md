# Carbide

A pure-Rust, DAG-evaluated spreadsheet where cells are not stuck being squares and interactive widgets turn workbooks into live models.

## Highlights

- **Four tessellating cell lattices** — **Squares**, **Hexagons** (pointy- and flat-top), **Triangles**, and **Voronoi** (with Delaunay-correct adjacency, draggable/freezable seeds, and persisted seed geometry).
- **Carbide formula language + multi-engine core** — Excel-style formulas with `LET` / `LAMBDA` / `LETREC`, dynamic arrays & spill, lattice neighborhood functions (`NEIGHBORS`, `RADIUS`), `XLOOKUP` (including wildcards), `OFFSET` / `INDIRECT`, an optional **Carbide → Rust native transpiler** (`--features native`), and an optional **embedded Python engine** via PyO3 (`--features python`).
- **Programmable widgets & Stage Mode** — in-cell sliders, toggles, buttons, and progress bars wired straight into the reactive DAG, plus **Stage Mode** (`Ctrl+Shift+P`) to hide editing chrome for mock apps and games.
- **Pure-Rust native & WebAssembly front-end** — `apps/carbide-ui` (`egui`/`eframe`) compiles to both native desktop and `wasm32-unknown-unknown`, with `.crbd` workbook persistence (and `.tscl` load back-compat) plus browser `localStorage` autosave.

## Quick links

- [`docs/README.md`](./docs/README.md) — Sprint Loops Project Book (intents, work ledgers, sprint records, and [`docs/history/decisions-legacy.md`](./docs/history/decisions-legacy.md)).
- [`docs/carbide/README.md`](./docs/carbide/README.md) — Carbide formula language specification and CI-verified [`reference.md`](./docs/carbide/reference.md).
- [`docs/all-rust-roadmap.md`](./docs/all-rust-roadmap.md) — All-Rust + Carbide-to-Rust transpiler execution roadmap.
- [`PLAN.md`](./PLAN.md) — Long-term architecture vision.
- [`CLAUDE.md`](./CLAUDE.md) — Repository contributor and agent guidance.

## Building and running

Requires **Rust (stable)** (and optionally `wasm32-unknown-unknown` target + [Trunk](https://trunkrs.dev/) for WebAssembly, or CPython 3.12+ headers for `--features python`).

```bash
# Core workspace (carbide-core, carbide-tess, carbide-formula, carbide-store, carbide-cli)
cargo build --workspace
cargo test --workspace

# Optional formula engine features
cargo test -p carbide-formula --features native
cargo test -p carbide-formula --features python

# Pure-Rust GUI (native desktop)
cargo run --manifest-path apps/carbide-ui/Cargo.toml
cargo test --manifest-path apps/carbide-ui/Cargo.toml

# Pure-Rust GUI (WebAssembly)
cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown
```

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](./LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](./LICENSE-MIT))

at your option.
