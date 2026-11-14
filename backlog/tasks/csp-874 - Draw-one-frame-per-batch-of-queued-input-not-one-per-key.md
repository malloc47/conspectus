---
id: CSP-874
title: 'Draw one frame per batch of queued input, not one per key'
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
  - docs/adr/0024-tui-runtime-and-architecture.md
  - docs/adr/0085-tui-mvu-architecture.md
priority: high
type: bug
ordinal: 898000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Scrolling the TUI quickly over mosh stalls it for seconds, and it keeps moving after the operator stops. The TUI asks for no mouse mode, so the terminal turns each wheel event into cursor keys. Ghostty's alternate-scroll mode sends three per wheel tick by default (`mouse-scroll-multiplier`), more as macOS speeds up a fast spin, and a trackpad sends one per cell height of pixels. mosh delivers every key, in order; it never drops input.

The event loop (`drive_loop` in `src/tui/runtime.rs`) reads one event per pass and draws a full frame after each, so a backlog of N cursor moves costs N frames. At 184×85 a frame is 3.5–6 KB of output and takes about 5–7 ms, so the loop tops out around 140–200 keys/s, below what a fast scroll produces. The excess queues up and the screen works through it after the scroll ends. The cost grows with screen area: the same stream left 0.21 s of catch-up at 80×30 and 1.58 s at 184×85.

Baseline, measured 2026-11-13 with the release build and the operator's real data: the sessions view at 184×85, run under a local `mosh-server`/`mosh-client` pair with a UDP relay adding 80 ms round-trip and a 2 Mbps cap, fed `ESC O B`/`ESC O A` sweeps (15 down, 15 up) at 300 keys/s for 3 s, kept changing for 1.6–2.4 s after the last key. A 180-key burst written at once into a plain pty took 1.32 s to drain in the sessions view and 0.42 s in the mux view.

Conspectus moves exactly one row per cursor key, and mosh delivers each key once (both verified), so the rows per notch are the terminal's setting. The rest of the stall is this loop.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When several input events are already queued, the loop handles all of them before it draws, so a backlog of cursor moves costs one frame; a runtime test feeds a queued burst and counts the frames drawn
- [x] #2 Each queued key still resolves through `action_for_event` and dispatches in order, so a batch leaves the app in the same state as the same keys handled one at a time, including a key that opens a modal followed by keys the modal handles
- [x] #3 The frame drawn after a batch keeps the selected row in view, however far the batch moved it
- [x] #4 While keys keep arriving, the screen still redraws at least every 100 ms, so a long scroll shows progress instead of freezing
- [x] #5 A bracketed paste inside a batch types into the focused field exactly as it does today
- [x] #6 Repeating the baseline measurement in the description, the screen settles within 250 ms of the last key, and the final summary records the before and after numbers
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirmed against HEAD: `drive_loop` (src/tui/runtime.rs) still polls and reads one event per pass and draws a frame at the top of every pass, so N queued keys cost N frames.
2. Make the loop testable without a terminal: a `Console` trait (poll, read, draw, now) in `src/tui/runtime/console.rs`, implemented by `DefaultTerminal`; `LoopMode` names its console type; a scripted console (virtual clock, scripted events, TestBackend frames) is the test double.
3. Write the failing runtime tests first (frames per queued burst, same state as one-at-a-time, a modal opened inside a batch takes the following keys, the frame keeps the selection in view, redraw at least every 100 ms under a continuous stream, a bracketed paste inside a batch).
4. `drive_loop`: after the first event of a pass, keep reading while `poll(0)` says another is queued and the frame budget (50 ms) has not run out, resolving each event through `action_for_event` against the state the previous key left; stop the batch on quit; draw once.
5. Check frames.sh frames byte-identical, a modal-opening key inside a batch with `conspectus tui --snapshot`, and the pty burst measurement (before: release build of cc56c7c5) on a copy of the state directory (numbers only).
6. ADR (next free number) for the batching rule; operations.md/CHANGELOG note; just check; commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: runtime tests in `src/tui/runtime/loop_tests.rs` drive the real `drive_loop` over a scripted console (virtual clock, frame counter). With the loop temporarily put back to one event per pass, the five frame-counting tests fail (60 queued keys drew 60 frames); with the 50 ms frame budget removed, the 100 ms test fails (one gap of about 2 s). `frames.sh` before/after: nine frames byte-identical. `tui --snapshot --snapshot-keys 'jjj?jj'` on the showcase fixture: the help page opens and the two keys after `?` scroll it, the selection stays on the third row. `just check` passes on the tree (5760 tests).
Measurement (release build, sessions view, 184x85, operator's real data on a COPY of the state and cache dirs, numbers only; harness `/tmp/csp-fu/wheel/pty_burst.py` and `mosh_burst.py`, not committed): 900 keys, `ESC O B`/`ESC O A` sweeps of 15 at 300 keys/s for 3 s. Through mosh-server/mosh-client behind a UDP relay (80 ms round trip, 2 Mbps): settles 1.46 and 2.13 s after the last key before, 0.144, 0.162, and 0.163 s after. Straight into a pty: 0.98, 2.57, 2.17, 2.00 s before; 0.006, 0.014, 0.006 s after. A 180-key burst written at once: 0.68 and 0.99 s before, under 10 ms after. The mux view was not measured on real data: that needs the host tmux server, which these runs keep off.

Method, to rebuild the harnesses (not committed; the repository has no place for measurement tools and the runs read an operator's real data): in Python with `pty`, run through a memory-capped scope with a wall-clock limit. Direct: spawn `conspectus tui --view sessions --no-resume-view` in a 85x184 pty on COPIES of the state and cache directories (tmux and forge discovery off), wait for output to go quiet, type `ESC O B` x15 then `ESC O A` x15, repeating, at 300 keys/s for 3 s while reading the output, and report the time from the last key to the last output chunk over 64 bytes (byte counts only; screen content is never printed or kept). Over mosh: the same TUI as the command of `mosh-server new -i 127.0.0.1`, a `mosh-client` in the pty, and a UDP relay between them that delays each direction 40 ms and serializes packets at 250 kB/s (2 Mbps).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The event loop (`drive_loop`) now handles the event that arrived and every event already queued behind it before it draws, so a backlog of cursor keys costs one frame. Each event resolves through `action_for_event` against the state the one before left (a key that opens a menu is followed by keys the menu takes; a bracketed paste types into its field; `r` anywhere in the batch forces the preview reload of the next pass). A batch ends when the queue empties, when the app quits, or after `FRAME_BUDGET` (50 ms), so a stream that never pauses still draws at least every 100 ms. The loop reaches the terminal through a `Console` trait (poll, read, draw, now) that `DefaultTerminal` implements and a mode names as `LoopMode::Terminal`, so the tests run the real loop over a scripted console. ADR 0236, `docs/operations.md` ("Scrolling quickly"), and the changelog record it.

Verified: eight loop tests (frames per burst, same state as one at a time, a menu opened inside a batch, selection kept in view after a 150-row batch, a frame at least every 100 ms under a continuous stream, a paste inside a batch, a quit inside a batch, keys that arrive alone each draw); frames.sh byte-identical; `just check` passes.

Before and after, sessions view at 184x85 on the operator's data (release build, numbers only): through mosh behind an 80 ms, 2 Mbps relay the screen settled 1.46 to 2.13 s after the last key before and 0.14 to 0.16 s after; straight into a pty, 1.0 to 2.6 s before and under 15 ms after; a 180-key burst written at once, 0.7 to 1.0 s before and under 10 ms after. Not measured: the mux view on real data (needs the host tmux server).

Visible changes: none in what a frame shows. Regressions: none known. Consistency wins: none.
<!-- SECTION:FINAL_SUMMARY:END -->
