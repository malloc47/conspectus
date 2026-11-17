---
id: CSP-892
title: >-
  TUI: an agent that stops to wait, finish, or fail reads unread until read, and
  read marks persist
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17 01:02'
updated_date: '2026-11-17 01:57'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: medium
ordinal: 968000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator asked (2026-11-16) for an agent that goes from working to a stop to read bold, as unread, with a way to mark it read and to clear every unread mark; reattaching to an agent reads it too. ADR 0214 (CSP-828) already marks a `ready` turn bold until it is seen, from attach evidence or the cursor's rest, remembered only in the running TUI, so a restart shows every finished turn unread again. The operator chose: the stops that count are waiting (permission, question, plan, input), ready, and error; read marks persist in a state file (the TUI's existing `tui-state.json`, ADR 0143 category 2); the gestures are TUI keys and menu entries (the task after this one). The user-facing words become unread and read.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A live cell that is waiting, ready, or in error reads unread (its mark bold) until the operator has read the stop that began its state, by attaching to its mux after the stop or by the cursor resting on its session row (sessions with no mux), as a finished turn does today; a mux's mark is bold while any agent counted on it is unread in the state the mark shows
- [x] #2 An agent that has never run (ready with nothing finished) and a state that began before the TUI first saw it as working are decided and recorded in the ADR, and tests cover sparse and ambiguous cases (an agent with no session, no mux, a state that repeats, a stop that follows a stop)
- [x] #3 Read marks are saved in tui-state.json (merged by the later stop time with what another TUI wrote, skipped when unchanged, written by the existing persist path), loaded at start, and bounded in size; a missing, unreadable, or newer file costs only the marks
- [x] #4 The detail pane, glossary, and docs/tui-columns.md say unread and read, with their tests and example frames regenerated; ADR 0214 and 0231 are amended by a new ADR and docs/design.md says it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
An agent whose live cell is waiting, ready, or in error now reads unread until its stop has been read, and its mark (`!`, `›`, `×`, and Pinned's words) draws bold; a mux's mark is bold while an agent counted in the state it shows is unread (the mux's unread is a count per state). Reading is ADR 0214's evidence (a client attached to the mux at or after the stop began, the operator's return to the TUI's session, the cursor resting on a session with no mux) applied to the time the shown state began, and `working`, compacting, starting, exited, and unknown are never unread. The marks of agent sessions are saved in `tui-state.json` as `read_marks` (node id to when the latest read stop began), loaded at start, merged on every write with the marks another TUI wrote (the later stop wins), bounded to the newest 2,000, read leniently (a bad entry costs only itself), written at the next tick, at quit, and at a gesture; a `--no-resume-view` run reads and writes nothing. The detail pane, glossary, and help legend say unread and read; `docs/tui-columns.md`, the operations guide, the design document, and the changelog follow, and ADR 0245 (amending 0214 and 0231) records the decisions, among them that a `ready` with no evidence stays unread and that pane agents' reads stay in memory. Verified by the full suite (5,900 tests) including new tests for stops in every state across attach and rest evidence, the state file's round trip, merge, bound, and lenient reads, and a render test that the waiting and error marks go bold and plain with the shape and color unchanged; and by the pre-main gate (just check, cargo test, the separation rehearsal, just plugins, nextest with no conspectus on PATH, backlog doctor).
<!-- SECTION:FINAL_SUMMARY:END -->
