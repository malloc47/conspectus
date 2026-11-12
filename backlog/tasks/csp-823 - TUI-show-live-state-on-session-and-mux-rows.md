---
id: CSP-823
title: 'TUI: show live state on session and mux rows'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
labels:
  - live-state
  - ls-tui
  - needs-decision
milestone: m-26
dependencies:
  - CSP-814
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 909000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The point of the feature is to see state at a glance. Where it appears (a new column beside the mux-link cell, replacing the session kind glyph, or a word in mixed views), which shapes mean what, and whether `working` animates are the operator's to choose: the common `●◐○` set collides with session kind, PR, and refresh marks, and the braille spinner already means "refresh pending".

Scope: Per the operator's decision in `docs/plans/live-state/tui-options.md`: the column schema entry, vetted one-cell glyphs (ADR 0154) and words (ADR 0145), theme roles for each state (ADR 0153), the density modes, and the `docs/tui-columns.md` rows, glyph table, state-word table, and regenerated frames. Reuses the row models filled by the plumbing story.

TUI changes: Adds the cell to Sessions, Mux, PRs, Pinned, and History as decided; list every frame change under Regressions and Consistency wins.

Tests and checks: `conspectus tui --snapshot` at normal and narrow widths in each view, `tests/tui_columns_doc.rs`, the consistency suites for glyphs, state, text, and freshness.

Blockers: Operator decisions 1, 6, 10 and the plumbing story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pending the operator's decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
