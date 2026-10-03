---
id: CSP-181
title: Same-line session preview switch
status: Done
assignee: []
created_date: '2026-05-19 23:23'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-166
ordinal: 408000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: narrow-mode stacking (terminal width < 100 cols)
  landed in the polish pass — the body switches from a
  horizontal split to a vertical stack at the threshold. The
  remaining locked behavior changed after operator feedback:
  session rows now stay one physical line tall and use remaining
  horizontal space after the mux indicator for a dim same-line
  `last_message_preview`, cropping or omitting it when width is
  tight.
- Tests: `cargo test tui --all-targets`.
- Blockers: `CSP-166` v1 slice.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Removed the second-line selected/recent preview rows;
`src/tui/ui.rs` appends the graph-resident preview to each
session row only when width remains after the fixed cells.
Left-tree auto-scroll now tracks one physical row per visible
row again.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-004`
