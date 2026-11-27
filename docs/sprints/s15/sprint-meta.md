# Sprint 15 Meta

- **Sprint number:** 15
- **Book schema version:** 2
- **Start timestamp:** 2026-11-27T01:10:33Z
- **End timestamp:** 2026-11-27T01:54:48Z
- **Model:** Gemini 4 Neon
- **Bundle version:** 0.22.0
- **Exit status:** success
- **Token count:** (filled at Loop Phase if observable)
- **Summary:** Modernize workspace and UI dependencies (`hashbrown 0.17`, `glam 0.34`, `thiserror 2`, `pyo3 0.30`, `libloading 0.9`, `rfd 0.18` superseding ADR-003, `egui`/`eframe 0.37`), align MSRVs, and expand Dependabot (`INT-0004`).
- **Intents:** [INT-0004](../../intents/INT-0004-dependency-and-toolchain-modernization.md)
- **Completion evidence:** INT-0004 realized (T-001..T-004 + 1532787): upgraded hashbrown 0.17, glam 0.34, thiserror 2, pyo3 0.30, libloading 0.9, rfd 0.18 (superseding ADR-003), egui/eframe 0.37; aligned MSRVs 1.88/1.95; expanded dependabot.yml; all local targets + 6/6 CI jobs (#45924350359) green
- **Checkpoint:** https://github.com/crussella0129/carbide/pull/198
