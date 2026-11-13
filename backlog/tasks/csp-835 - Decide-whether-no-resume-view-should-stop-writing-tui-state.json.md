---
id: CSP-835
title: Decide whether --no-resume-view should stop writing tui-state.json
status: To Do
assignee: []
created_date: '2026-11-12 01:58'
labels:
  - tui
milestone: m-23
dependencies: []
priority: low
type: task
ordinal: 892000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus tui --no-resume-view` skips reopening the persisted view (and, since CSP-702, each view's expanded and selected rows), but the run still enables persistence: `event_loop` and `static_event_loop` in `src/tui/runtime.rs` call `resumed_app`, which sets the `TuiStateCache`, so view switches, settings changes, and the end of the run write `$XDG_STATE_HOME/conspectus/tui-state.json` as usual, including `last_view`. The help text used to say the flag "does not write the sidecar on view switches", and comments in `src/tui/app.rs` and `src/tui/runtime.rs` said the cache was left unset under the flag; CSP-702 corrected the help and the comments to match the code, and `docs/operations.md` ("TUI state", Opt-outs) now says the run's own changes are still written.

Decide which is wanted. Option A (today): the flag only skips what is reopened; a scripted run's last view and rows become the next interactive run's. Option B: the flag also skips every write (no `enable_view_persistence`), which then also skips restoring sort, filters, and groupings, unless restoring is split from writing (a read-only cache mode on `App`).

Scope: the decision, the code for option B if chosen, `--help`, `docs/operations.md`, and a CLI test in `tests/cli_tui_state_invariants.rs` that pins the chosen behavior (a `--no-resume-view` run cannot be driven headless today; `tui --fixture` with scripted keys or a unit test through `resumed_app` may do). Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The chosen behavior of --no-resume-view for writes is recorded (ADR or task summary) and matches --help and docs/operations.md
- [ ] #2 A test pins whether a --no-resume-view run writes tui-state.json
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
