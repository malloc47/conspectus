---
id: CSP-185
title: Throttle and freshen mux pane-capture previews
status: To Do
assignee: []
created_date: '2026-05-20 01:56'
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
