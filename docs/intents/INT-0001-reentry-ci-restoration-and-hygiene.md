# INT-0001 — Re-entry CI restoration, ADR-015 URL completion, and doc/lint hygiene

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0001
- **State:** realized
- **Work evidence:** [Sprint 12 build plan](../sprints/s12/sprint-plans/build-plan.md), [Sprint 12 test plan](../sprints/s12/sprint-plans/test-plan.md), [T-001..T-004](../work/tasks.md)
- **Completion evidence:** [T-001 completion](../work/completed-tasks.md#t-001-sprint-12), [T-002 completion](../work/completed-tasks.md#t-002-sprint-12), [T-003 completion](../work/completed-tasks.md#t-003-sprint-12), [T-004 completion](../work/completed-tasks.md#t-004-sprint-12)
- **Code evidence:** [apps/carbide-ui/src/app.rs](../../apps/carbide-ui/src/app.rs), [Cargo.toml](../../Cargo.toml), [crates/carbide-cli/src/main.rs](../../crates/carbide-cli/src/main.rs), [.github/workflows/ci.yml](../../.github/workflows/ci.yml)
- **Test evidence:** [Sprint 12 test report](../sprints/s12/sprint-tests/test-report.md)
- **Documentation evidence:** [README.md](../../README.md), [CLAUDE.md](../../CLAUDE.md)

## Intent
Restore Carbide to a green, accurate baseline on the newly deployed Sprint Loops Book v2 substrate (`dev -> main`) after a 6-month hiatus, before resuming feature development or larger refactors.

In scope:
1. Fix all 21 Rust 1.99 `float_literal_f32_fallback` future-incompatibility errors in `apps/carbide-ui/src/app.rs` (rust-lang/rust#154024) so `cargo clippy --all-targets -- -D warnings` and `cargo check --target wasm32-unknown-unknown` pass with zero warnings.
2. Remove the 4 stale `#[allow(dead_code)]` attributes in `apps/carbide-ui/src/app.rs` on already-live Voronoi fade/pop-out helpers (`SEED_FADE_OUTER`, `SEED_FADE_MAX_ALPHA`, `WIDGET_POP_OUT_OFFSET`, `pop_out_widget_center`).
3. Complete ADR-015 step 3 by updating the three deliberately preserved `crussella0129/tescellate` repository URLs (`Cargo.toml`, `CLAUDE.md`, and the About window hyperlink in `apps/carbide-ui/src/app.rs`) to `crussella0129/carbide` now that the GitHub repository rename has completed.
4. Add `dev` to `on.push.branches` in `.github/workflows/ci.yml` so pushes to the Book v2 `work` branch run the full CI suite automatically.
5. Refresh obviously stale Phase-0 / Electron-era documentation in `README.md`, `CLAUDE.md`, `crates/carbide-cli/src/main.rs`, and the `CarbideApp` About dialog prose so newcomers and agents see the actual all-Rust egui/eframe + 4-lattice (`Square`, `Hex`, `Triangle`, `Voronoi`) architecture and working build/test commands.

Out of scope (explicitly deferred to `INT-0002`–`INT-0005` for user prioritization):
- Any behavioral change to formula evaluation, DAG recompute, `.crbd` v2 persistence, or rendering.
- Splitting `apps/carbide-ui/src/app.rs` (7,433 lines) into submodules (`INT-0002`).
- Lattice UI unification or Voronoi formatting/notes/copy-paste parity (`INT-0003`).
- Major dependency upgrades (`egui`/`eframe`, `rfd`, `pyo3`) or MSRV changes (`INT-0004`).
- Deleting the 72 legacy merged remote branches on GitHub (requires explicit user approval).

## Acceptance criteria
1. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p carbide-formula --features native --all-targets -- -D warnings`, and `cargo test --workspace` all exit 0 with zero warnings.
2. `cargo fmt --manifest-path apps/carbide-ui/Cargo.toml -- --check`, `cargo clippy --manifest-path apps/carbide-ui/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path apps/carbide-ui/Cargo.toml`, and `cargo build --manifest-path apps/carbide-ui/Cargo.toml --target wasm32-unknown-unknown` all exit 0 with zero warnings.
3. Zero `#[allow(dead_code)]` attributes remain in `apps/carbide-ui/src/app.rs`.
4. Zero `crussella0129/tescellate` URLs remain in `Cargo.toml`, `CLAUDE.md`, or `apps/carbide-ui/src/app.rs` (all three point to `crussella0129/carbide`).
5. `.github/workflows/ci.yml` triggers `push` on both `main` and `dev`.
6. `README.md`, `CLAUDE.md`, `crates/carbide-cli/src/main.rs`, and the About dialog text reflect the current pure-Rust workspace + `apps/carbide-ui` stack (no stale "Phase 0 — nothing user-visible works yet" or `apps/desktop` Electron claims) and no runtime behavior or `.crbd` compatibility regresses.

## Rationale
After 6 months, Rust stable advanced to 1.99.1 and elevated `float_literal_f32_fallback` into a default-on future-incompatibility warning, which breaks the `webui` CI job (`-D warnings`) on every PR. Completing this small, zero-ambiguity restoration sprint alongside the Book v2 substrate deployment unblocks green CI checkpoints on `dev -> main` and cleans up the already-approved ADR-015 post-rename follow-up and misleading docs before asking the user which larger architectural or feature sprint (`INT-0002`–`INT-0005`) to prioritize next.

## Alternatives
- **Stop immediately after substrate migration without fixing CI:** rejected because the Book v2 migration itself could not open a green `dev -> main` checkpoint PR while `webui` clippy fails with 21 errors.
- **Bundle `app.rs` modularization or `egui` upgrades into this sprint:** rejected because those carry architectural and dependency-churn tradeoffs that deserve explicit user prioritization first.

## Consequences
- `dev` becomes the clean, CI-verified `work` branch under Book v2 contract 4.
- Larger improvements are cleanly documented as `proposed` intent chapters (`INT-0002`–`INT-0005`) in the Book for immediate selection once this checkpoint PR merges.

## Transition history
- 2026-11-26: created as `proposed` during Sprint 12 6-month project assessment.
- 2026-11-26: transitioned `proposed` → `planned` with Sprint 12 build/test plans and tasks `T-001`–`T-004`.
- 2026-11-26: transitioned `planned` → `active` at Sprint 12 Build Phase start.
- 2026-11-26: transitioned `active` → `realized` after Sprint 12 verification (639 local tests, wasm32 zero-warning build, and GitHub Actions CI run `45866040577` all green).
