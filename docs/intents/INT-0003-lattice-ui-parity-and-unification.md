# INT-0003 — Lattice UI parity and Voronoi operational unification

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0003
- **State:** active
- **Work evidence:** [Sprint 14 build-plan.md](../sprints/s14/sprint-plans/build-plan.md) (`T-001`–`T-004`)
- **Completion evidence:** none
- **Code evidence:** none
- **Test evidence:** none
- **Documentation evidence:** none

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
