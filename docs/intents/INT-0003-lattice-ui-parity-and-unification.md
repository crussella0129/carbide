# INT-0003 — Lattice UI parity and Voronoi operational unification

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0003
- **State:** realized
- **Work evidence:** [Sprint 14 build-plan.md](../sprints/s14/sprint-plans/build-plan.md) (`T-001`–`T-004`)
- **Completion evidence:** [T-001 completion](../work/completed-tasks.md#t-001-sprint-14), [T-002 completion](../work/completed-tasks.md#t-002-sprint-14), [T-003 completion](../work/completed-tasks.md#t-003-sprint-14), [T-004 completion](../work/completed-tasks.md#t-004-sprint-14)
- **Code evidence:** [clipboard.rs](../../apps/carbide-ui/src/clipboard.rs), [state_io.rs](../../apps/carbide-ui/src/state_io.rs), [app/cells.rs](../../apps/carbide-ui/src/app/cells.rs), [app/commands.rs](../../apps/carbide-ui/src/app/commands.rs), [app/dialogs.rs](../../apps/carbide-ui/src/app/dialogs.rs), [app/edit.rs](../../apps/carbide-ui/src/app/edit.rs), [app/helpers.rs](../../apps/carbide-ui/src/app/helpers.rs), [app/io.rs](../../apps/carbide-ui/src/app/io.rs), [app/render_voronoi.rs](../../apps/carbide-ui/src/app/render_voronoi.rs), [app/voronoi_helpers.rs](../../apps/carbide-ui/src/app/voronoi_helpers.rs) (commits `08d20f6`, `975a893`, `0705c9e`, `abe5e58`)
- **Test evidence:** [Sprint 14 test-report.md](../sprints/s14/sprint-tests/test-report.md) (646/646 tests green, GitHub CI run #45877964117 6/6 `success`)
- **Documentation evidence:** [Sprint 14 research-report.md](../sprints/s14/sprint-research/research-report.md)

## Intent
Complete the lattice-unification arc deferred in ADR-013: bring Voronoi to full operational parity with square/hex/triangle (per-cell formatting, notes, copy/paste, and multi-cell marquee `Selection.extra` operations) and consolidate duplicated per-lattice state/dispatch paths in `CarbideApp`.

## Acceptance criteria
1. Voronoi supports cell formatting (`voronoi_formats` persisted in `UiSnapshot`), notes (`voronoi_notes` persisted in `UiSnapshot`), and copy/cut/paste without breaking older `.crbd` v2 snapshots (`#[serde(default)]`).
2. Multi-cell Voronoi marquee selections (`Selection.extra`) operate through format, clear, widget toggle, and copy pipelines rather than ignoring extras via `primary_cells()`.
3. Shared helpers/traits eliminate copy-pasted dispatch arms across square, hex, triangle, and Voronoi where semantics agree, covered by unit tests.

## Rationale
Square, hex, triangle, and Voronoi currently duplicate state maps (`square_widgets`, `hex_widgets`, `triangle_widgets`, `voronoi_widgets`, etc.) and have uneven feature coverage (Voronoi lacks formats, notes, and marquee-aware bulk actions). Closing this gap fulfills ADR-005 / ADR-013 before adding more tessellations (e.g., parallelogram or Archimedean tilings).

## Alternatives
- **Leave Voronoi as a partial surface and jump straight to new features:** keeps feature disparity and makes future lattices repeat the same four-way boilerplate.

## Consequences
- `UiSnapshot` gains `#[serde(default)]` fields (`voronoi_formats`, `voronoi_notes`); `.crbd` `FORMAT_VERSION` remains 2 (backward-compatible opaque `ui.json` extension per ADR-001).
- Best sequenced after `INT-0002` so per-lattice code is cleanly separated first.

## Transition history
- 2026-11-26: created as `proposed` during Sprint 12 6-month project assessment.
- 2026-11-26: `proposed` → `planned` (Sprint 14 Plan Phase: `T-001`–`T-004` in `docs/sprints/s14/sprint-plans/build-plan.md`).
- 2026-11-26: `planned` → `active` (Sprint 14 Build Phase started).
- 2026-11-26: `active` → `realized` (Sprint 14 Loop Phase: `T-001`–`T-004` complete, 646/646 tests + 6/6 CI jobs green, Test Critic verdict `proceed-with-caveats` resolved).
