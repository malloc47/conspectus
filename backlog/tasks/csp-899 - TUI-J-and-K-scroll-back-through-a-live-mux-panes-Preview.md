---
id: CSP-899
title: 'TUI: J and K scroll back through a live mux pane''s Preview'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-18 08:53'
updated_date: '2026-11-18 09:20'
labels:
  - tui
dependencies: []
ordinal: 975000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On a mux row (Mux view, or any row whose Preview is a live pane capture) K does nothing: the Preview body is built as only the last screenful of the capture (the bottom height rows), so there is nothing above it to scroll to, though the capture holds 100 lines of scrollback. The operator expects J and K to scroll just the Preview, as they do for a transcript.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 On a live mux pane Preview, K scrolls back through the captured scrollback and J returns toward the newest line; it opens on the newest output, and J at the newest line does nothing
- [x] #2 A refresh of the capture keeps the place the operator scrolled to, counted from the newest line
- [x] #3 A Preview with a failure banner or a Related row's evidence above its capture is unchanged; so are transcripts, files, and git previews
- [x] #4 A capture shorter than the zone shows as before; the scrollbar appears when the capture overflows
- [x] #5 docs/operations.md says it; tests cover the capture's scrollback, the newest line, the banner case, and a refresh
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A live mux pane's Preview now holds the whole capture (the last 100 lines of scrollback and the screen), newest row last, and scrolls from the newest line like a transcript: it opens on the newest output, J toward it stops there, K goes back a row a press, and the place is kept as a distance from the newest line through a refresh. The cause was `format_preview_for_mux` cutting the capture to the zone's bottom rows, so the scroll region had nothing above it. A failure banner or Related-row evidence above a capture still cuts it to its newest rows (`PreviewView::fitted_under_a_header`), and a content search shows the rows around its matches as before (ADR 0250). Verified: unit tests (the whole capture newest last, scrolling back to the first row and returning, J at the newest row, a refresh keeping the place, the header case); the real TUI in a private tmux server over a pane with 300 lines of output, in the operator's stacked layout: K moved 290..300 back to 287..297, J returned, the place held through a refresh, and 150 presses reached row 181; the full pre-main gate passed on the first run.
<!-- SECTION:FINAL_SUMMARY:END -->
