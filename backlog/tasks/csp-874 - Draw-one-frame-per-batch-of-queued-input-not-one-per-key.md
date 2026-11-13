---
id: CSP-874
title: 'Draw one frame per batch of queued input, not one per key'
status: To Do
assignee: []
created_date: '2026-11-13 20:40'
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
- [ ] #1 When several input events are already queued, the loop handles all of them before it draws, so a backlog of cursor moves costs one frame; a runtime test feeds a queued burst and counts the frames drawn
- [ ] #2 Each queued key still resolves through `action_for_event` and dispatches in order, so a batch leaves the app in the same state as the same keys handled one at a time, including a key that opens a modal followed by keys the modal handles
- [ ] #3 The frame drawn after a batch keeps the selected row in view, however far the batch moved it
- [ ] #4 While keys keep arriving, the screen still redraws at least every 100 ms, so a long scroll shows progress instead of freezing
- [ ] #5 A bracketed paste inside a batch types into the focused field exactly as it does today
- [ ] #6 Repeating the baseline measurement in the description, the screen settles within 250 ms of the last key, and the final summary records the before and after numbers
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
