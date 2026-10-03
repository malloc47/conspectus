---
id: CSP-261
title: Filtered-zero empty frame
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 457000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped — `empty_left_panel_text` in `src/tui/ui.rs`
renders `No rows match <chips>.\nPress \`F\` to clear
filters, \`f\` to edit.` when the active filter drops every
row. The status bar continues to render the active chips so
the operator sees exactly which predicates are in play. The
hint pointed to a stale `v` binding until a follow-up commit
realigned it with the controls overlay accelerator (`f`).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-012`
