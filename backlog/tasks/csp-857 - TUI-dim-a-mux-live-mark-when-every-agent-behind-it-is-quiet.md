---
id: CSP-857
title: 'TUI: dim a mux live mark when every agent behind it is quiet'
status: To Do
assignee: []
created_date: '2026-11-12 22:42'
labels:
  - live-state
  - ls-tui
  - needs-decision
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 942000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0209 draws a mux's rolled-up live mark never dimmed, because a mux row carries only per-state counts (`LiveCounts`) and no confidence. A mux whose only working agent has gone quiet therefore shows a bright `»` while that agent's own row shows it dimmed. Dimming the mux mark when every agent in its dominant state is quiet needs a quiet count per state on the mux row. CSP-823 and CSP-824 left this open; it changes ADR 0209 decision 3, so it needs the operator's decision.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A mux whose agents in its dominant state are all quiet draws its mark dimmed, and one with a fresh agent in that state draws it bright
- [ ] #2 The mux row model carries what the rule needs, filled by the same pass that fills the counts, and refills in place
- [ ] #3 docs/tui-columns.md and an ADR amending ADR 0209 describe the rule; tests cover both cases
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
