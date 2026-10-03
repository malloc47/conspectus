---
id: CSP-583
title: Returning from tmux redraws the TUI without waiting for the refresh (ADR 0108)
status: Done
assignee: []
created_date: '2026-10-02 23:54'
labels:
  - h-handoff-latency
milestone: m-18
dependencies: []
ordinal: 551000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Symptom: after the ADR 0104 nudge, a plain detach left the screen
  blank while the TUI nudged the daemon's `mux` and `harness` classes,
  fetched the snapshot, and rebuilt rows on the UI thread. That took
  about half a second, longer when a git rebuild held the daemon's
  writer lock, and seconds with no daemon.
- Tests: `tui::runtime` `worker_ledger`, `tui::app` hand-off reducer,
  `tui::ui` pending-row rendering.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-10-02): attach returns dispatch
`Msg::HandoffReturned`; `LiveMode` answers with a nudged background
worker. Until it lands, the mux row and linked session rows render
dimmed with a spinner in the attach-glyph cell, and the mux's
preview is recaptured right away. Worker results carry a generation
(`WorkerLedger`), so an older result can't overwrite a newer one.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-HANDOFF-LATENCY-001`
