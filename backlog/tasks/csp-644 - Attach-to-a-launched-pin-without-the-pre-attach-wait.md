---
id: CSP-644
title: Attach to a launched pin without the pre-attach wait
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-20 03:33'
updated_date: '2026-10-21 14:39'
labels:
  - tui
  - pins
dependencies: []
references:
  - docs/adr/0103-launches-keep-failed-panes.md
  - docs/adr/0127-launches-keep-the-tui-drawn-behind-a-spinner.md
priority: high
type: enhancement
ordinal: 702000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Launching a pin, `mux new`, or `mux launch` from the TUI starts the harness in a detached tmux session and attaches only after two blocking steps: the launch subprocess watches the new pane (500 ms fresh, 1500 ms resume; ADR 0103), then the TUI runs the pre-attach refresh (two daemon nudges behind the writer lock, or a full local discovery without `conspectus serve`; ADR 0127). The harness runs the whole time, so the operator attaches after its first output has already scrolled past. The operator reports the delay is still noticeable with `conspectus serve` running.

ADR 0127 already called the pre-attach refresh largely redundant: the attach's `HandoffReturned` triggers a background refresh when it returns, the attach target is resolved before the launch, and the attach-availability check queries tmux directly. The fresh-launch watch only catches exec-level failures, which `remain-on-exit failed` already keeps visible as a dead pane that the attach-return log entry reports. The resume watch still drives the fresh-launch fallback, so it stays.

Out of scope: holding the process until a client attaches (tmux `wait-for` plus a `client-attached` hook), which needs its own capability entry and ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pin launch, `mux new`, and `mux launch` from the TUI attach right after the launch subprocess returns, with no refresh in between; the refresh runs after the attach returns, as it does today
- [ ] #2 Launch log entries, failure reporting, the attach-availability check, and the launching row's dim-and-spinner mark (ADR 0127) behave as before
- [x] #3 A fresh launch (no resume argv) returns without a watch window; a launch that dies at once still shows as a dead pane on attach and in the attach-return log entry
- [x] #4 A resume launch keeps its watch window and fresh-launch fallback
- [x] #5 An ADR amends 0103 and 0127, and `docs/design.md` reflects the change if it describes either step
- [x] #6 Tests cover the fresh-launch path without a watch, the retained resume watch, and the launch flow attaching without a pre-attach refresh
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `src/cli/launch_watch.rs`: drop the fresh-launch watch. `spawn_watched` takes only the resume window (1.5 s); a fresh launch, including the fallback after a failed resume, returns once `tmux new-session` succeeds. `mux launch` spawns without a watch.
2. `src/cli/pin.rs`: reword the attach `NoTarget` error, which now covers a process that exited cleanly right after launch rather than only a race.
3. `src/tui/runtime/launch.rs`: on success, pin launch, `mux new`, and `mux launch` attach right after the subprocess returns; the attach's `HandoffReturned` drives the background refresh. Paths that end without an attach (failed subprocess, no attach target, session not attachable) keep the blocking refresh before reporting, so the tree reflects any dead session removed or worktree created.
4. Tests: rework `launch_watch_tests.rs` (fresh launch skips the watch even when the pane is dead; resume watch, fallback, and vanished-session-on-resume still covered); add a runtime test that the launch flow does not refresh before attaching if a seam exists.
5. ADR 0133 amending 0103 (item 2, fresh window) and 0127 (item 6, order); update `docs/design.md` and `docs/operations.md`.
6. `just check`, then commit on main with CSP-644.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented per plan. Pin path's post-launch decision is extracted as `after_pin_launch` (pure; reads tmux directly) so the attach-without-refresh behavior is unit-tested; `mux new` / `mux launch` follow the same shape. Recorded as ADR 0133.

Validation: `just check` passes (fmt, clippy -D warnings, cargo test, nextest 2520/2520, cargo doc, git diff --check). Live CLI timing on a private tmux socket: fresh `mux launch --no-attach` returned in 16 ms (was >= 500 ms) and the harness printed its first line after the command returned; a missing binary returned in 16 ms with exit 0 and left a dead pane (status 127).

Not yet exercised: an interactive TUI pin launch / `mux new` / `mux launch` attach (needs a real terminal). AC #1 and #2 stay open for the operator's live check.
<!-- SECTION:NOTES:END -->
