---
id: CSP-569
title: Retire `SessionViewerAction`
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 578000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: ADR 0019's external-viewer trait has one implementation
  (`ClaudeHistoryViewer`) behind a one-element
  `[&dyn SessionViewerAction; 1]`, and the native viewer (ADR 0052)
  is now the default.
- Plan: collapse it to a function; amend ADR 0019.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`resolve_viewer_target` is a straight-line check with the
same outcomes; ADR 0019 amended with the rationale and the
data-driven extension point for `CSP-332`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-016`
