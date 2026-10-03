---
id: CSP-463
title: Extract shared TUI/output row-assembly helpers
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 94000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-04 (commit `efd16ed`). Migrated into
  `src/tui/rows/mod.rs`: `struct AgentData`, `agent_row(depth,
  …)`, `mux_indicator`, `session_matches_filter`,
  `collect_agent_mux_candidate_counts`. Deleted the 3
  verbatim copies from `rows/{union,prs,forks}.rs` and the
  2 copies from `output/{prs,forks}.rs`. `rows/mux.rs`'s
  near-twin `agent_row` stays put per the story scope (it
  takes a different input struct). Net −361 / +159 across
  6 files. `collect_agent_mux_candidate_counts` retires
  permanently in CSP-467 (interim home per story).
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-002`
