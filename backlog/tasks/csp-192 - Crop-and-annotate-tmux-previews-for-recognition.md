---
id: CSP-192
title: Crop and annotate tmux previews for recognition
status: To Do
assignee: []
created_date: '2026-05-20 03:28'
updated_date: '2026-10-31 08:15'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-168
ordinal: 440000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: make the mux preview behave like a recognition surface,
  not a raw dump. Prefer the bottom N visible lines from
  `capture-pane`, preserve wrapping enough to resemble the
  terminal pane, and add a compact preview header such as
  `preview · tmux · captured 2s ago`. Surface stale, disabled,
  and failed capture states in that header when possible.
- Tests: preview adapter tests for bottom-line cropping,
  configurable line budget, stale/fresh labels, and failed
  capture labels; Ratatui snapshots for long and short captures.
- Blockers: `CSP-168` v1 slice; overlaps `CSP-185` freshness
  work and should be planned with it.
- **slice landed**: mux previews now use a compact separator
  carrying the display target and capture freshness when cached,
  and captured pane text is cropped to the bottom lines available
  in the preview zone. Configurable budgets and stale/failure
  header variants remain open with `CSP-185`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-016`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the Preview's divider is `tuikit::frame::SectionRule`, which carries no summary today, so re-check this story's "slice landed" claim (target and freshness on the separator) before relying on it. Stale, disabled, and failed states would be the rule's summary or the body's placeholder note (ADR 0190). See the CSP-185 note.
<!-- SECTION:NOTES:END -->
