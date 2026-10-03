---
id: CSP-319
title: TUI responsive-layout design and breakpoints
status: To Do
assignee: []
created_date: '2026-06-01 03:32'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-190
  - CSP-313
ordinal: 424000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: codify the layout breakpoints the TUI uses across all
  views so the detail-pane explorer (and the rest of the TUI)
  renders predictably across terminal sizes. Decide and
  document: (a) the width / height where left + right panes
  stack vertically instead of side-by-side, (b) the width where
  the right pane auto-expands when it gains focus, (c) the
  width where the right pane is hidden entirely and the
  operator cycles to it via a tab affordance, (d) the
  narrow-pane content-drop rules called out under
  "Narrow-terminal Behavior" in `docs/tui-detail-mockup.md`.
  Likely deliverables: a design note in `docs/` (promoted to an
  ADR if the choices are cross-cutting) plus the implementation
  that applies the rules uniformly across sessions / mux /
  union / prs / forks views and the detail explorer.
- Tests: render tests at representative terminal sizes covering
  each breakpoint transition; snapshot regression for the
  stack-vs-split, expand-on-focus, and hide-and-tab behaviors;
  keymap coverage for the tab affordance when the right pane is
  hidden.
- Blockers: `CSP-190` (contextual status bar — the breakpoint
  rules need to play nicely with the contextual status zone),
  `CSP-313` v1 slice (so the detail-pane explorer's needs are
  concrete before thresholds are picked).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-033`
