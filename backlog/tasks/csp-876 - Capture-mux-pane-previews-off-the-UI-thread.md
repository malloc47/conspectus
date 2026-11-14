---
id: CSP-876
title: Capture mux pane previews off the UI thread
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 20:40'
updated_date: '2026-11-14 22:16'
labels:
  - h-scroll
  - tui
milestone: m-11
dependencies: []
references:
  - src/tui/runtime.rs
  - src/discovery/tmux/mod.rs
  - docs/adr/0024-tui-runtime-and-architecture.md
priority: medium
type: bug
ordinal: 900000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`LiveMode::load_preview` (`src/tui/runtime.rs`) loads every preview on a worker thread except a mux pane's. A pane capture runs `tmux display-message … ; capture-pane -p -J -e -S -100` synchronously through the executor's mux op, before the frame draws.

Measured 2026-11-13 against the operator's tmux server, one capture takes 5–10 ms. Each cursor move onto a live mux row blocks input for that long, and scrolling the mux view stacks those waits on top of the frame cost (CSP-874). The call also has no timeout (`Command::output`), so a tmux server that stops answering freezes the whole TUI, not just the preview.

CSP-185 lists background capture as item (4) of a wider mux-preview task that also covers a refresh interval, a freshness label, and a snapshot test. This task takes that item alone so it can land with the scroll fixes. ADR 0024 runs background work on threads over `mpsc`, with no async runtime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Selecting a live mux row never runs `tmux` on the UI thread: with a mux backend whose capture blocks, a test shows keys still being handled and frames still drawn while the capture is in flight
- [x] #2 At most one capture per pane is in flight at a time, and a capture that lands after the cursor has left its row is stored without replacing the preview on screen
- [x] #3 A pane captured before keeps showing its last capture until the new one lands, and a pane never captured shows the loading placeholder
- [x] #4 Preview state still changes only through the reducer, and the capture still runs through the mux backend, so no program, capability entry, or write path is added
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirmed against HEAD: `LiveMode::load_preview` still captures a pane synchronously (`load_preview_now` -> `execute_mux_op` -> `capture_via`), on the UI thread before the frame draws; the call has no timeout.
2. `LiveMode` holds its mux backend as `Arc<dyn MuxBackend>` (the trait is `Send + Sync`; the content search driver already shares one this way). `PreviewLoads::start` resolves the pane's native id on the UI thread (a snapshot read), then captures through `capture_via` on a worker thread and sends the result through the same channel as the other loads; `drain` hands it to the reducer as `Msg::SetPreview`. A pane with no target answers `NoTarget` at once with no tmux call. No new program, capability entry, or write path.
3. At most one capture per pane: `PreviewLoads` tracks the panes it is capturing and ignores a second start, besides the store's loading mark the planner already honors. A capture is never stopped: one that lands after the cursor left is stored under its own source and replaces nothing on screen; a pane captured before keeps its last capture until the new one lands; a pane never captured shows the placeholder (the renderer's existing rule).
4. Tests first: a mux backend whose capture blocks (a gate the test opens): `PreviewLoads::start` returns while it blocks; the loop over the scripted console handles keys and draws frames while it is in flight and shows the placeholder; one capture per pane; late capture stored without changing the screen; previous capture kept until the new one lands; no-target pane makes no tmux call.
5. frames.sh byte-identical; operations.md/changelog; ADR 0237 gets no change (decision 6 said the capture moves in its own story), the preview module doc and ADR 0112 section 5 note the worker; `just check`; commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: `src/tui/runtime/previews_tests.rs` holds the capture behind a gate (`src/tui/runtime/blocked_tmux.rs`, a mux backend whose capture waits until the test opens it): `PreviewLoads::start` returns while the capture blocks and the result lands only through `drain`; three starts for one pane make one capture, and a second pane makes its own; a capture that lands after the cursor left (`keep_only` another row) is stored under its pane and no other preview changes; a pane captured before keeps "editor capture 1" while the second capture is held, then shows "editor capture 2"; a mux with no pane answers NoTarget with no tmux call. `loop_tests::keys_are_handled_and_frames_drawn_while_a_pane_capture_is_in_flight` runs the real loop over a scripted console with blocked captures: two captures are in flight (the first pane, then the one the cursor rests on), none finished, the cursor moved, more than eight frames drew after the first key, the placeholder shows; after the gate opens both captures are stored and the screen shows the cursor's pane. With the capture put back on the calling thread five of these fail (20 s). The capture goes through `capture_via` on the mux backend and reaches the reducer as `Msg::SetPreview` in `drain`; `exec::command` is untouched, so `tests/capabilities_static.rs` and the traced capabilities test pass in `just check`.
Real binary (release, private tmux server under a scratch TMUX_TMPDIR with three sessions, a `tmux` wrapper on PATH that sleeps 6 s before `capture-pane`, mux view in a 40x120 pty; /tmp/csp-fu/wheel/pty_mux_stall.py, not committed): a `j` pressed 3 s after the first screen produced output 3.16 s later before the change and 0.001 s later after. frames.sh: nine frames byte-identical to the pre-CSP-874 baseline. `just check` passes (5776 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`LiveMode` now holds its mux backend as an `Arc<dyn MuxBackend>` and `PreviewLoads::start` captures a pane through `capture_via` on a worker thread, with every other source: the result returns over the same channel and reaches the reducer as `Msg::SetPreview` when the loop drains it. The pane's native id is read from the snapshot on the UI thread; a mux with no pane answers `NoTarget` at once with no tmux call; `PreviewLoads` records the panes it is capturing so a pane has at most one capture in flight; a capture is never stopped, and one that lands after the cursor left is stored under its own pane and changes nothing on screen. A pane captured before keeps its last capture until the new one lands, and a pane never captured shows the loading placeholder (the renderer's existing rule). No program, capability entry, or write path was added; no timeout was added either (ADR 0238 says why: it belongs to the process layer). ADR 0238, operations.md, the design doc, and the changelog record it; ADR 0237 decision 6 now points at it.

Verified: five capture tests over a gated backend, one end-to-end loop test with captures in flight (keys handled, frames drawn, placeholder shown, late captures stored), all five failing when the capture is put back on the UI thread; a real-binary check with a stuck `tmux` on a private server (a key pressed during a 6 s capture produced output after 3.16 s before and 1 ms after); frames.sh byte-identical; `just check` passes (5776 tests).

Visible changes: none in what a frame shows; the mux view no longer waits on tmux between keys. Regressions: none known. Consistency wins: none. Not measured: the 5-10 ms per-move cost on the operator's own tmux server (that needs the host server); a tmux that never answers still leaves its pane on the placeholder or its last capture, with its worker waiting (no timeout, by decision).
<!-- SECTION:FINAL_SUMMARY:END -->
