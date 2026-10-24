---
id: CSP-664
title: Record muxes in history
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 12:45'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-662
  - CSP-663
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: high
type: feature
ordinal: 722000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every mux Conspectus observes should be recorded so it can be rebuilt after it dies, however it was created. This replaces `pin create` / `pin adopt` as the way a mux becomes durable, and replaces the ADR 0058 pin-binding sidecar as the record of a mux's last session.

- Scope: writers at the end of the daemon's mux-class cycle and of in-process discovery; only probes this process ran count; matching order (same incarnation renames the record; a ghost with the same key comes back with a new incarnation; otherwise a new record); hosted sessions (10 most recent) from resolved `linked_to_mux`, dropping sessions without activity after a recreated mux's `created`; launch paths (`mux new`, `mux launch`, rebuild) write the record and argv with source `conspectus`; observed `pane_start_command` recorded verbatim with source `observed`; `[history.ignore]` mux globs.
- Tests: tests for an external rename, a name reused after death, a mux that disappears during a probe failure (stays live or unknown, never gone), hosted-session rollover, and ignore globs.
- Manual checks: create a mux by hand with plain `tmux new -s`, rename it outside Conspectus, kill it, and confirm one record follows it through and shows as gone.
- Blockers: the history store and mux probe status tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Any mux observed by the daemon or by in-process discovery gets a history record, unless an ignore glob matches
- [x] #2 An external rename updates the existing record, and a recreated name re-lives its ghost record
- [x] #3 A failed or skipped probe never marks a mux gone, and a stale snapshot never moves `last_seen`
- [x] #4 Each mux record lists its recent hosted sessions and its launch argv with its source
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. history::recorder turns fresh listed probes into mux observations (incarnation, start command, hosted sessions from resolved evidence, ignore globs, orchestrator owner).
2. Wire the writers: daemon cycle, in-process CLI/TUI discovery, mux new / mux launch.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every mux a successful, fresh probe lists is recorded unless an ignore glob matches, with its incarnation, `pane_start_command` (verbatim, source `observed`), harness, orchestrator owner, and up to ten hosted sessions taken from resolved evidence other than history's own fallback. `mux new` and `mux launch` record the argv they used (source `conspectus`) before any probe, and an observed command never overwrites it. Writers run in the daemon cycle, in-process CLI and TUI discovery, and `cli lifecycle refresh`; a failed or skipped probe and reused data record nothing. Verified by recorder, merge, and projection unit tests plus `tests/cli_history.rs`, where a hand-made tmux session renamed outside Conspectus and then killed stays one record that shows as gone. Manual check done in a private tmux server.
<!-- SECTION:FINAL_SUMMARY:END -->
