---
id: CSP-492
title: 'Retire or permanently bless the `[tui].sessions_grouping` legacy alias'
status: To Do
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-adr
milestone: m-11
dependencies: []
ordinal: 177000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ADR 0031 promised the alias survives "until a follow-on ADR
  retires it"; that ADR never happened and the alias path is live at
  `src/config.rs:624`. Decide retire-with-deprecation-warning vs bless
  as permanent, record it as the promised follow-on ADR, and remove or
  annotate the code path accordingly. Consider alongside whether
  `[table.<rows>]` (ADR 0021) and `[tui.views.<name>]` (ADR 0031)
  should converge when `H-EXT` config work touches this area.
- Tests: config parse tests for whichever outcome (warning emitted, or
  alias documented as permanent).
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-ADR-003`
