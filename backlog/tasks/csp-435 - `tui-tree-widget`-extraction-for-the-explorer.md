---
id: CSP-435
title: '`tui-tree-widget` extraction for the explorer'
status: Done
assignee: []
created_date: '2026-06-19 00:59'
updated_date: '2026-10-31 08:16'
labels:
  - h-widg
milestone: m-11
dependencies: []
ordinal: 374000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `src/tui/explorer.rs` (3272 LOC) carries an
  in-tree tree state machine alongside domain-aware rendering.
  `tui-tree-widget` 0.24 (MIT, ratatui 0.30) would let the
  expand / collapse / selection state move upstream; only
  domain rendering stays in-tree.
- Scope: deferred. Re-evaluate when the explorer is next on
  the audit list (post CSP-418) or when a defect traces back to
  the tree state machine specifically.
- Blockers: explorer re-think on the agenda.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-011`

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed as won't do: the framework's own tree and explorer replaced the premise. The explorer is a flat list of zones (properties, then links under one section rule) with a drill stack, not a nested tree, so `tui-tree-widget`'s expand and collapse state did not fit it (operator review, CSP-755). Its state machine moved to `tuikit::explorer::Explorer` over the host's `ExplorerSource` (CSP-756, ADR 0188), and the left pane's tree state, folds, and cursor kept by identity are `tuikit::tree::TreeState` and `TreeView` (CSP-750, ADR 0182). No dependency was added (ADR 0152). Recorded by the TUI framework's backlog impact review (CSP-766, `docs/plans/tui-framework/backlog-impact.md`).
<!-- SECTION:FINAL_SUMMARY:END -->
