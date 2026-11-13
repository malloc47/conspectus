---
id: CSP-876
title: Capture mux pane previews off the UI thread
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
- [ ] #1 Selecting a live mux row never runs `tmux` on the UI thread: with a mux backend whose capture blocks, a test shows keys still being handled and frames still drawn while the capture is in flight
- [ ] #2 At most one capture per pane is in flight at a time, and a capture that lands after the cursor has left its row is stored without replacing the preview on screen
- [ ] #3 A pane captured before keeps showing its last capture until the new one lands, and a pane never captured shows the loading placeholder
- [ ] #4 Preview state still changes only through the reducer, and the capture still runs through the mux backend, so no program, capability entry, or write path is added
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
