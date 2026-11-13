---
id: CSP-875
title: Start a preview load only once the cursor rests on the row
status: To Do
assignee: []
created_date: '2026-11-13 20:40'
labels:
  - h-scroll
  - h-preview
  - tui
milestone: m-11
dependencies: []
references:
  - src/tui/runtime.rs
  - src/tui/preview/mod.rs
  - docs/adr/0112-node-specific-previews.md
  - docs/adr/0024-tui-runtime-and-architecture.md
priority: medium
type: bug
ordinal: 899000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every cursor move starts a preview load for the row it lands on, even when the cursor is only passing through. `plan_preview_load` (`src/tui/runtime.rs`) loads again whenever the selection's source differs from the one last shown, cached or not.

- A repo row runs four git commands: `symbolic-ref`, `remote -v`, `rev-parse`, and `log -n 30`.
- A checkout row runs `git status`.
- A session row reads its transcript.

Each runs on a worker thread that keeps going after the cursor moves on. A live mux row instead runs `tmux capture-pane` on the UI thread; moving that off the UI thread is its own task.

Measured 2026-11-13 with strace on the release build: a 90-key down/up sweep started 134–198 git processes in the sessions and mux views, about two per key, all for rows the operator only scrolled past. Those processes compete for CPU with the UI thread just when it is working through a scroll backlog (CSP-874), and the stall is worst on a loaded machine.

The content search already waits for its query to rest before it scans (`search_overlay_tick`).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Moving the cursor through rows without stopping starts no preview load for the rows it passes, and the row it comes to rest on starts its load within 150 ms of the last key
- [ ] #2 A row passed during a move shows its cached preview if it has one and the loading placeholder otherwise, as today
- [ ] #3 A load still running when the cursor leaves its row starts no further subprocesses for that row
- [ ] #4 `r`, a hard refresh, the mux preview interval refresh, and the reuse of loaded pull-request previews behave as they do today
- [ ] #5 Tests cover a burst of moves that starts one load, for the row the cursor rests on, and a single move followed by a pause that loads the new row
- [ ] #6 Repeating the 90-key sweep from the description starts git only for the rows the cursor rested on, and the final summary records the before and after process counts
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
