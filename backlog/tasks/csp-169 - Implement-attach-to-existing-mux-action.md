---
id: CSP-169
title: Implement attach-to-existing-mux action
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 396000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`a` key bound. `src/tui/actions.rs` resolves the
attach target from the current selection — preferred mux for
an agent session, the candidate's mux for an
`AgentSessionMuxCandidate` child row, or the mux directly for
mux node rows. Pure resolver returns either an `AttachTarget`
or a typed `AttachDisabled` reason; the runtime surfaces the
reason in the status bar and stays in the TUI when attach
isn't available. On success, the runtime restores the
terminal and `exec()`s `tmux attach-session -t <native_id>`,
so the conspectus process becomes the tmux client (Unix
only). Six unit tests cover the resolver across attached /
ambiguous / un-muxed / candidate-row / unsupported-row /
no-selection paths. Remaining work tracked as `T8-008`:
surface attach-disabled status messages with the
yellow-chip styling intended for `CSP-180` and verify
real-tmux attach via a manual script.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-010`
