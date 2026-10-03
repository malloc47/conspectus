---
id: CSP-168
title: Add mux live-preview capture adapter
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 395000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`TmuxRunner` gained a `capture_pane(target)` method
with a default `TmuxCaptureOutcome::Unsupported` impl so
existing runners didn't have to change. `SystemTmux`
implements `tmux capture-pane -p -J -t <target>` and maps
failure modes (binary missing, no server, target missing,
other) to typed outcomes. `FakeTmux::with_capture` lets
tests register canned per-target responses.
`src/tui/preview.rs` exposes a `PreviewStore` cache keyed
by `MuxSessionId` plus a `capture_via(runner, native_id)`
helper that translates `TmuxCaptureOutcome` →
`PreviewContent`. The runtime calls capture synchronously
after each event when the selection's mux target has
changed (skipped when `live_preview_enabled` is false) and
dispatches the new `Msg::SetMuxPreview` into the reducer.
The UI's right-panel preview reads from the cache; muxed
rows show the captured pane content, "loading mux
preview…" before the first capture, or a typed error
surface (no target / unavailable / failed). Six unit tests
cover the adapter + cache.
Throttling on the configured `mux_preview_interval`, async
background capture, freshness markers, and snapshot tests
over the preview render move to `CSP-185` (filed alongside).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-009`
