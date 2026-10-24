---
id: CSP-692
title: 'Make the pin, history, and rebuild TUI consistent with the rest of the TUI'
status: Done
assignee: []
created_date: '2026-10-23 02:27'
updated_date: '2026-10-23 04:28'
labels:
  - history-pins
  - tui
milestone: m-23
dependencies: []
priority: high
type: feature
ordinal: 750000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Manual testing of the first pin-slot TUI found about two dozen defects that share a few root causes: new views and menus reimplemented gestures instead of reusing the established ones, mutating actions skip the shared freeze, refresh, and follow-focus lifecycle, menus differ by the view they open from, the right pane isn't treated as an action target, forms lost behavior from the pin forms they replaced, and developer vocabulary leaked into the interface. This parent tracks the corrective work and the invariant tests that keep the TUI uniform.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Corrected the TUI inconsistencies found in manual testing (CSP-692.01 to .05) and added invariant tests so views, menus, lifecycle, and text stay uniform.
<!-- SECTION:FINAL_SUMMARY:END -->
