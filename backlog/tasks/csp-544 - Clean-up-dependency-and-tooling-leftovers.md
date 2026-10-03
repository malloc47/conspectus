---
id: CSP-544
title: Clean up dependency and tooling leftovers
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 615000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Progress (2026-09-30, `CSP-557` / `CSP-561`): `proptest`,
  `rstest`, `_selection_display`, and the "CSP-088 wave N" comments
  are gone. Remaining: confirm `tui-pantry`, and resolve `pre-commit`
  without a config.
- Scope: `proptest` and `rstest` are dev-dependencies with zero uses;
  `tui-pantry` serves only `examples/pantry.rs` (confirm the `CSP-424`
  "go" still holds); the flake ships `pre-commit` but there is no
  `.pre-commit-config.yaml` (ADR 0007 promised hooks); dead helper
  `_selection_display` (`src/cli/mod.rs:769`); historical
  "CSP-088 wave N" comments in `src/cli/mod.rs`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-013`
