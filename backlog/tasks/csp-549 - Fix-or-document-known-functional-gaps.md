---
id: CSP-549
title: Fix or document known functional gaps
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: low
ordinal: 620000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `CSP-526` (`atelier exec claude` panes show
  "No agent"), `CSP-382` (non-default tmux socket discovery),
  `CSP-175` (inline picker for ambiguous mux candidates; `m` now belongs
  to the Mux menu, see `CSP-534.02`), `CSP-505.03` (per-cycle
  resolve/publish cost), zellij mutation capabilities. Also: Atelier
  fork-index paths are used verbatim (`src/discovery/atelier.rs:377`),
  so a fork's checkouts get workspace-relative roots and repo ids
  (`.atelier/forks/alpha/repo-a`, `repo-a`) that never join the
  canonical nodes git discovery produces. Existing Atelier snapshot
  tests pin this shape, so fixing it means re-blessing them.
- Blockers: per item.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-018`
