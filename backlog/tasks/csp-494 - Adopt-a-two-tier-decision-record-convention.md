---
id: CSP-494
title: Adopt a two-tier decision-record convention
status: To Do
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-adr
milestone: m-11
dependencies: []
ordinal: 179000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ~25 of the 84 ADRs are pixel-level UI decisions
  (0034, 0061–0063, 0071/0072, 0074/0075, …) whose supersession upkeep
  demonstrably lags, diluting the ~30 load-bearing records. Record a
  convention ADR: full ADRs for model/persistence/dependency/workflow
  decisions; a lighter `docs/design-notes/` (or a `Tier: UI` header
  with relaxed supersession expectations) for view polish, using
  ADR 0078's rubric style as the model. Migrating existing UI ADRs is
  optional; stopping the dilution is the point. Update the CLAUDE.md
  Decision Records section to reference the tiers.
- Tests: docs-only; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-ADR-005`
