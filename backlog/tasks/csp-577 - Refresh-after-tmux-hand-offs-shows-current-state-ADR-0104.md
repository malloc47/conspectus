---
id: CSP-577
title: Refresh after tmux hand-offs shows current state (ADR 0104)
status: Done
assignee: []
created_date: '2026-10-01 18:04'
labels:
  - h-pin-fix
milestone: m-11
dependencies: []
ordinal: 345000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: nudge the daemon's `mux` + `harness` classes before the
  refresh that follows attach, pin launch, mux new/launch and
  renames, and on `r`; fall back to a local rebuild and report when
  the daemon refresh fails.
- Tests: existing suites; the nudge is an IPC call against a live
  daemon, measured at about 0.2 s per class with `conspectus
  refresh --class`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`refresh_after_mux_handoff` and `DaemonNudge` in
`tui::runtime`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-FIX-004`
