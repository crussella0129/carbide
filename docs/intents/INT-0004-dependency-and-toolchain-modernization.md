# INT-0004 — Dependency, toolchain, and CI modernization

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0004
- **State:** realized
- **Work evidence:** [Sprint 15 build-plan.md](../sprints/s15/sprint-plans/build-plan.md) (`T-001`–`T-004`)
- **Completion evidence:** [T-001 completion](../work/completed-tasks.md#t-001-sprint-15), [T-002 completion](../work/completed-tasks.md#t-002-sprint-15), [T-003 completion](../work/completed-tasks.md#t-003-sprint-15), [T-004 completion](../work/completed-tasks.md#t-004-sprint-15)
- **Code evidence:** [Cargo.toml](../../Cargo.toml), [crates/carbide-formula/Cargo.toml](../../crates/carbide-formula/Cargo.toml), [crates/carbide-formula/src/python/mod.rs](../../crates/carbide-formula/src/python/mod.rs), [apps/carbide-ui/Cargo.toml](../../apps/carbide-ui/Cargo.toml), [apps/carbide-ui/src/ribbon.rs](../../apps/carbide-ui/src/ribbon.rs), [apps/carbide-ui/src/app/mod.rs](../../apps/carbide-ui/src/app/mod.rs), [.github/dependabot.yml](../../.github/dependabot.yml) (commits `23a3ee5`, `3f798c3`, `34e523b`, `be8b12d`, `1532787`)
- **Test evidence:** [Sprint 15 test-report.md](../sprints/s15/sprint-tests/test-report.md) (all local targets + GitHub CI run #45924350359 6/6 `success`)
- **Documentation evidence:** [Sprint 15 research-report.md](../sprints/s15/sprint-research/research-report.md), [ADR-003 superseded note](../history/decisions-legacy.md)

## Intent
Audit and modernize Carbide's 6-month-old dependency graph and automation config: evaluate upgrading `egui`/`eframe` (0.29), `rfd` (0.14 — revisiting the ADR-003 wasm `gtk3` workaround), `pyo3` (0.23), `hashbrown` (0.14), `glam` (0.27), and workspace MSRV (`rust-version = "1.75"`), and expand `.github/dependabot.yml` to cover the Cargo workspaces (`/` and `/apps/carbide-ui`).

## Acceptance criteria
1. Every intentional major dependency bump is verified across all CI jobs (lint, Ubuntu + Windows workspace tests, `native`, `python`, and `webui` native + wasm32).
2. `Cargo.toml` `rust-version` matches the actual minimum toolchain required by the dependency tree.
3. `.github/dependabot.yml` covers `github-actions` plus both Cargo manifests targeting `dev` (or documents why any ecosystem is intentionally excluded).

## Rationale
`apps/carbide-ui` still pins `egui`/`eframe 0.29` and `rfd 0.14`, and the root workspace pins `hashbrown 0.14`, `glam 0.27`, and `pyo3 0.23` while claiming MSRV `1.75`. Periodic, bounded dependency refreshes prevent large breaking jumps later (especially for `egui` and `pyo3` on newer Python versions).

## Alternatives
- **Pin current versions indefinitely:** avoids short-term migration work but risks security advisories, Python 3.14+ incompatibility in `pyo3 0.23`, and steeper `egui` upgrades later.

## Consequences
- May require small API adaptations for `egui`/`eframe` and `pyo3` major versions; should be run as its own focused sprint once the user approves the upgrade scope.

## Transition history
- 2026-11-26: created as `proposed` during Sprint 12 6-month project assessment.
- 2026-11-26: `proposed` → `planned` (Sprint 15 Plan Phase: `T-001`–`T-004` in `docs/sprints/s15/sprint-plans/build-plan.md`).
- 2026-11-26: `planned` → `active` (Sprint 15 Build Phase started).
- 2026-11-26: `active` → `realized` (Sprint 15 Loop Phase: `T-001`–`T-004` + MSRV lint cleanup `1532787` complete, all local targets + 6/6 CI jobs `#45924350359` green, Test Critic C-001 resolved).
