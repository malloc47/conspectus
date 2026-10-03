---
id: CSP-355
title: Per-tool expand on click
status: To Do
assignee: []
created_date: '2026-06-03 13:09'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-344
  - CSP-354
ordinal: 221000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: in tool-detail Summary or Truncated mode, clicking
  the chip pill of an individual tool turn temporarily
  expands *that* turn to full detail while leaving the
  global tool detail level unchanged. Pressing the same key
  or clicking again collapses. Inspired by claude-history's
  per-message expand UX.
- State: `expanded_tool_turns: BTreeSet<usize>` on
  `ViewerState` (turn indices currently expanded). Cycling
  `t` clears the per-turn overrides.
- Renderer: per-turn render consults the override set; an
  expanded turn renders at `ToolDetail::Full` regardless of
  the global level.
- Folds into `CSP-344` (mouse) for the click
  target. Without mouse, a `Tab`/`o`-style "expand cursor"
  keybind can drive it from the keyboard (overlap with the
  selection story).
- Tests: reducer for set toggling; widget assertions that
  an expanded turn renders more lines than its peers at
  the same global level.
- Blockers: `CSP-344` (mouse) and
  `CSP-354` (selection cursor for keyboard
  expand).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-016`
