---
id: CSP-693
title: 'Follow up on pin, rebuild plan, and refresh defects from hands-on testing'
status: To Do
assignee: []
created_date: '2026-10-23 16:02'
updated_date: '2026-10-23 16:04'
labels:
  - history-pins
  - tui
milestone: m-23
dependencies: []
priority: high
type: feature
ordinal: 756000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Second round of manual testing of the pin, history, and rebuild work found: stopped pins read gone instead of showing the hollow pin glyph and ghost styling has disappeared, p pins but does not unpin, the pin edit form has shifting and clipped text and unexplained labels, the rebuild plan can't be shown for a running mux and can't be copied, an unpinned stopped mux still shows in the Mux view, and refreshes after starting, detaching, or killing a mux show stale state. These are tracked together with invariant tests for the patterns behind them.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Assumptions made without asking: stopped pins read as a hollow glyph plus dim ghost style, with the word stopped on pin rows instead of gone; p unpins immediately; the Mux view hides unpinned stopped muxes by default.
<!-- SECTION:NOTES:END -->
