---
id: CSP-494
title: Adopt a two-tier decision-record convention
status: To Do
assignee: []
created_date: '2026-07-01 22:56'
updated_date: '2026-10-31 08:15'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): there are now 200 ADRs, and 51 of them (0150 to 0200) record the TUI framework extraction, most of them at the level this story calls pixel-level UI. The framework also keeps its own decision log (`crates/tuikit/docs/decisions/`, record 0070), which restates the framework's part of those ADRs and becomes canonical once the crate leaves the repository (`crates/tuikit/docs/separation.md`, step 6). A two-tier convention should say where a UI decision goes now that framework decisions have a home of their own: a framework control's rule in the framework's log, a Conspectus screen's in an ADR or the lighter tier.
<!-- SECTION:NOTES:END -->
