---
id: CSP-185
title: Throttle and freshen mux pane-capture previews
status: To Do
assignee: []
created_date: '2026-05-20 01:56'
updated_date: '2026-10-31 08:15'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-168
ordinal: 411000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: the v1 CSP-168 cut runs `tmux capture-pane`
  synchronously on every selection change and never re-runs
  until the next change. Add (1) a `mux_preview_interval`-
  driven refresh so a stable selection still gets fresher
  captures, (2) a freshness label ("captured Ns ago") in the
  right-panel preview header, (3) a snapshot test over the
  preview render with a fake runner so the layout stays
  locked, and (4) background-thread execution per ADR 0024
  so capture never blocks input. The background piece
  overlaps `CSP-184`; consider folding the two into a single
  background-work pass.
- Blockers: `CSP-168` v1 slice. Best done alongside `CSP-184`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-009`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the Preview's divider is the framework's `tuikit::frame::SectionRule` (`preview_divider_line` in `src/tui/ui/preview_pane.rs`, ADR 0188) and carries no summary today, so a "captured Ns ago" label would be its summary, worded with `tuikit::age` (ADR 0157). The old sectioned detail's `captured` field went with CSP-710. Capture and preview loading are still in `src/tui/preview/`; the framework adds no worker (the runtime stays in Conspectus, ADR 0200).
<!-- SECTION:NOTES:END -->
