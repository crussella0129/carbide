# INT-0004 — Dependency, toolchain, and CI modernization

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0004
- **State:** active
- **Work evidence:** [Sprint 15 build-plan.md](../sprints/s15/sprint-plans/build-plan.md) (`T-001`–`T-004`)
- **Completion evidence:** none
- **Code evidence:** none
- **Test evidence:** none
- **Documentation evidence:** none

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
