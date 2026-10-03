---
id: CSP-578
title: TUI message log for operation outcomes (ADR 0105)
status: Done
assignee: []
created_date: '2026-10-01 18:04'
labels:
  - h-pin-fix
milestone: m-11
dependencies: []
ordinal: 346000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: in-memory `MessageLog` fed by `Msg::Report` from launches,
  attaches, viewer, renames, pin-store and worktree executors,
  daemon nudges and refresh failures; `!` Messages overlay with full
  command records; persistent unseen-failure chip; failure banner in
  the affected row's Preview; dead-pane check after attach returns.
- Tests: `tui::messages` (capacity, unseen, latest failure,
  repeats, full text), `tui::widgets::messages` (navigation,
  render), `tui::ui` (preview banner, status chip, info report),
  `tui::runtime` (`!` key, `launch_entry` levels).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Launch subprocess output is kept in full instead of the
first 180 characters of stderr; failure toasts are replaced by
the status message plus chip.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-FIX-005`
