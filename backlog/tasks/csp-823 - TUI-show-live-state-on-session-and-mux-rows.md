---
id: CSP-823
title: 'TUI: show live state on session and mux rows'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 20:04'
labels:
  - live-state
  - ls-tui
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

Blockers: Operator decisions 1, 7, and 10 and the plumbing story.
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 items 1, 7, 10: Option A. A new always-filled one-cell column right after the age on session rows, mux rows (rolled up), nested agent rows, and the agent rows of the PRs view. Glyphs: '!' waiting, '»' working (also compacting and starting), '›' ready, '×' error (the shared failure mark), '?' unknown or stale (the existing can't-tell mark), '·' no evidence or exited. A quiet state dims the same glyph; shape never depends on color; colors are theme roles (ADR 0153); every glyph is vetted (ADR 0154). No animation. Pinned and History use words (waiting, working, ready, error), replacing 'in mux' when live state exists (CSP-829 owns the detail pane). Narrow widths and the density modes: decide the give-way order with the existing narrow-row rules and say so in docs/tui-columns.md. Unseen/seen styling and the header chip are separate stories.
<!-- SECTION:NOTES:END -->
