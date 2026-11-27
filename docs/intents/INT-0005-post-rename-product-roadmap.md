# INT-0005 — Post-rename product feature roadmap (awaiting prioritization)

<!-- sprint-loop-intent-v2 -->
- **Intent ID:** INT-0005
- **State:** proposed
- **Work evidence:** none
- **Completion evidence:** none
- **Code evidence:** none
- **Test evidence:** none
- **Documentation evidence:** none

## Intent
Capture the primary user-facing capabilities deferred at the end of v161 (ADR-014 / ADR-015) so the user can prioritize and split them into concrete sprint intents once re-entry improvements are settled:
1. **CSV / `.xlsx` import** (`csv` + `calamine` crates) — split into [INT-0006](INT-0006-csv-and-xlsx-import.md) for Sprint 16.
2. **Voronoi editing completeness** — add/delete seeds, bounds editing, and zoom-aware seed drag / widget pop-out (ADR-012 / ADR-014).
3. **Engine tiers in the GUI** — expose native Carbide compilation (`promote_native`) and Python engine cells directly in `apps/carbide-ui`.
4. **Headless CLI commands** — turn `crates/carbide-cli` from a `--help` stub into a usable `eval` / `validate` / `recompute` tool.
5. **Additional tessellations** — complete `LatticeKind::Parallelogram` (currently `None` in `LatticeHandle::for_kind`) and move toward Archimedean tilings (`PLAN.md` §3.2).

## Acceptance criteria
To be refined into individual child intents (`INT-0006+`) once the user chooses the next product direction.

## Rationale
Tracking these deferred product directions as a single umbrella `proposed` intent keeps the Book complete without prematurely committing sprint scope before the user weighs in after the 6-month break.

## Alternatives
- **Start implementing CSV/`.xlsx` import immediately:** premature because the user explicitly asked to assess the whole project and look for improvements *before* continuing building.

## Consequences
- This intent remains `proposed` (or gets split/superseded into specific feature intents) until the user picks a product priority.

## Transition history
- 2026-11-26: created as `proposed` during Sprint 12 6-month project assessment.
