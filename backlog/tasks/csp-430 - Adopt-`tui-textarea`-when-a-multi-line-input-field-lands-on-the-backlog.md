---
id: CSP-430
title: Adopt `tui-textarea` when a multi-line input field lands on the backlog
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies: []
ordinal: 369000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `widgets/input.rs` shells over `tui-input` for
  single-line entry. Multi-line input is unbuilt today; future
  candidates: pin `reason` notes, richer alias editing,
  PR-comment composer for forge surfaces. `tui-textarea` 0.7
  (MIT, ratatui 0.29+, pure state machine, 96% docs) is the
  canonical drop-in.
- Scope: deferred until a story names the surface that needs
  multi-line. Adopt the moment one does; do not preempt.
- Blockers: a downstream story that demands multi-line input.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-006`
