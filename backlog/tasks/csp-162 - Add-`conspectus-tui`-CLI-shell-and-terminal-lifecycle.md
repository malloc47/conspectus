---
id: CSP-162
title: Add `conspectus tui` CLI shell and terminal lifecycle
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 389000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus tui` subcommand registered with the full
locked flag surface (`--scan-root`, `--view`,
`--sessions-grouping`, `--sort`, `--refresh-interval`,
`--mux-preview-interval`, `--no-live-preview`, `--color`).
`src/tui/` module skeleton in place per ADR 0024: `mod.rs`
exposes `RunConfig` + `run`; `app.rs` carries the pure
reducer; `runtime.rs` owns the alt-screen / raw-mode lifecycle
and the event loop; `ui.rs` renders a placeholder frame for
downstream stories to replace. Pure reducer and event-
translation are unit-tested without a terminal. Integration
smoke tests cover `tui --help` and flag validation.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-003`
