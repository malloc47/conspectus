---
id: CSP-672
title: Add the rebuild engine and mux executor
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-658
  - CSP-667
  - CSP-668
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0098-pin-resume-preserves-launch-argv.md
  - docs/adr/0103-launches-keep-failed-panes.md
priority: high
type: feature
ordinal: 730000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rebuilding replaces `pin launch` and generalizes it: anything in history or a slot can be rebuilt, and the operator sees the steps first. The mux executor carries over the launch behavior pins have today (resume splicing, failed-resume fallback, agent-exited handling).

- Scope: a plan model (ordered steps, shown before running, printed by `--dry-run`); `conspectus rebuild <ref> [--dry-run] [--no-attach]` for a slot id, history id, or node reference; the mux executor: `new-session` with the effective argv (slot override > recorded > launch profile > harness default), resume spliced after the harness binary (ADR 0098), fresh-launch fallback (ADR 0103), attach; the agent-exited path (attach if the harness is running, else `send-keys` the built argv); non-harness muxes relaunch with recorded argv or a shell; orchestrator-owned muxes refused; the history record is written at launch; the pin launch code in `src/cli/pin.rs` is removed.
- Tests: plan and executor tests over fake tmux runners for fresh, resume, failed resume, agent-exited, non-harness, and orchestrator-owned cases; capability tests pass.
- Manual checks: kill a pinned claude mux and rebuild it, confirming the same name, argv, and resumed session.
- Blockers: the rebuild ADR, the graph projection ticket, and the slot store ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `rebuild` accepts a slot, history record, or node, and `--dry-run` prints the plan without running it
- [ ] #2 A ghost mux rebuilds with its name, cwd, effective argv, and last session resumed, falling back to a fresh launch when the resume is rejected
- [ ] #3 A live mux whose harness exited relaunches without ever typing into a running harness
- [ ] #4 Orchestrator-owned muxes refuse to rebuild with a message naming the owner
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
