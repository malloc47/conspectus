---
id: CSP-435
title: '`tui-tree-widget` extraction for the explorer'
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
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
