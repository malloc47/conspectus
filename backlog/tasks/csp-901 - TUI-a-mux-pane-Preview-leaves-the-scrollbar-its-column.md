---
id: CSP-901
title: 'TUI: a mux pane Preview leaves the scrollbar its column'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-18 10:52'
updated_date: '2026-11-18 11:09'
labels:
  - tui
dependencies:
  - CSP-899
ordinal: 977000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Since CSP-899 a live pane's Preview holds the whole capture and scrolls, so it nearly always overflows its zone and the region draws a scrollbar. The region wraps overflowing lines a cell narrower, beside the scrollbar, but the host laid the capture out at the zone's full width. Every row exactly as wide as the zone breaks off a one-cell row. Under smart wrap that hits every rule cut at the edge, so each of Claude Code's prompt rules takes two rows, the second a lone `─`. The operator reported it on 2026-11-18 alongside CSP-900. The transcript Preview already leaves the column free.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A mux pane Preview whose capture overflows the zone lays its rows out a cell narrower than the zone, so no row re-wraps beside the scrollbar and a rule cut at the edge takes one row
- [x] #2 A capture that fits the zone keeps every column
- [x] #3 A test renders an overflowing capture with a full-width rule and finds it on one row
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. A ui test renders an overflowing capture with a full-width rule and finds it on one row above the prompt; another shows a capture that fits keeps every column and an overflowing one is laid out a cell narrower.
2. In `format_preview_for_mux`, lay the capture out at the full width and, when its rows overflow the zone's height, again a cell narrower, as the region's own wrapping narrows beside the scrollbar.
3. CHANGELOG entry; no ADR, since this carries out ADR 0181's rule that rows drawn beside the scrollbar are laid out a cell narrower.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Validation: `just check` passes. The new test failed first with the reported shape (the rule, then a row holding a lone `─`, above the prompt). A snapshot of a live Claude Code capture shows both prompt rules on one row each. No ADR: the region already lays out overflowing rows a cell narrower (ADR 0181), and the transcript Preview leaves the column free; the mux path did not.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A mux pane's Preview lays its capture out a cell narrower than the zone when the rows overflow it, so no row re-wraps beside the scrollbar. Since CSP-899 the Preview holds the whole capture and nearly always scrolls, and every row as wide as the zone (above all, a rule smart wrap cut at the edge) broke a lone cell onto a row of its own: each of Claude Code's prompt rules took two rows. A capture that fits keeps every column. Verified by `mux_preview_rules_take_one_row_beside_the_scrollbar` and `mux_preview_that_fits_its_zone_keeps_every_column` (the first failing before the fix), `just check`, and a snapshot of a real Claude Code capture.
<!-- SECTION:FINAL_SUMMARY:END -->
