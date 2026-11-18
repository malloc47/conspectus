---
id: CSP-899
title: 'TUI: J and K scroll back through a live mux pane''s Preview'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-18 08:53'
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
- [ ] #1 On a live mux pane Preview, K scrolls back through the captured scrollback and J returns toward the newest line; it opens on the newest output, and J at the newest line does nothing
- [ ] #2 A refresh of the capture keeps the place the operator scrolled to, counted from the newest line, and goes back to the newest output when the Preview shows something else
- [ ] #3 A Preview with a failure banner or a Related row's evidence above its capture is unchanged; so are transcripts, files, and git previews
- [ ] #4 A capture shorter than the zone shows as before; the scrollbar appears when the capture overflows
- [ ] #5 docs/operations.md says it; tests cover the capture's scrollback, the newest line, the banner case, and a refresh
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
