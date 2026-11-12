---
id: CSP-810
title: 'Make `hook write` a fast, silent live-state writer'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 04:51'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-809
documentation:
  - docs/adr/0204-the-live-state-store-folds-agent-events-per-session.md
  - >-
    docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
  - docs/plans/live-state/research/01-claude-code.md
priority: high
type: enhancement
ordinal: 896000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code and Codex run `conspectus hook write` once per event, and a state event can fire twice per tool call. Today the command waits on the daemon's full resolve and `graph.bin` rewrite, the socket client has no timeout, and a bug could print to stdout (which Claude feeds to the model) or exit 2 (which blocks a tool). The live-state path has to be cheap and unable to disturb the agent.

Scope: Add a `HarnessAdapter` method that maps a payload to a neutral `LiveEvent` (the sibling of `hook_record_from_payload`), parsing payloads in memory against a typed whitelist (ADR 0206) so large `tool_response` bodies cost nothing to skip. Route `hook write` through the store: read `TMUX_PANE` directly, spawn tmux only on first sight of an entry, write the store before the existing attribution path, never open a socket for non-session-start events, print nothing on stdout or stderr, and exit 0 on every error. Add a read and write timeout to the daemon socket client so a stuck daemon cannot hang a session start. Add the `hook write` path to the traced capability test and update `docs/capabilities.toml` and ADR citations if needed.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: A timing test (median under 15 ms per event on the test machine; document the number, do not make CI flaky), stdout and stderr empty on success and on forced failures, exit code 0 on a corrupt store, a locked store, a missing tmux, and invalid JSON, a hung-daemon test for the new socket timeout, and the sentinel privacy test through the full command.

Blockers: The store.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `hook write` for a state event touches no socket and spawns no process after an entry's first sight
- [x] #2 `hook write` prints nothing and exits 0 on every error path, with a test per path
- [x] #3 The daemon socket client times out instead of hanging
- [x] #4 The capability tests cover `hook write`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Refactor: move `hook write` (args, /proc walk, tmux context, spool/ingest) out of src/cli/hook.rs into src/cli/hook_write.rs with no behavior change.
2. Adapter seam: add `HarnessAdapter::live_events_from_payload`, `live_snapshot_from_payload`, `live_reducer`, and `is_attribution_event` (defaults: no events, no snapshot, shared reducer, attribution true). Claude Code maps `SessionStart` to `SessionStarted` (compact source flagged) through a borrowed, whitelisted serde view, and attributes only `SessionStart`.
3. Socket client: `call_command` gains per-call read and write timeouts; `client_hook_ingest` uses a named 2 s bound. Unit test with a listener that never replies.
4. Writer: `hook write` reads stdin, builds a LiveContext (TMUX_PANE and $TMUX read directly, CLAUDE_PID else the /proc walk, pid start time), looks the entry up and spawns tmux only on first sight, applies events/snapshot with the 200 ms lock wait, then runs the unchanged attribution path only for attribution events. Silent on stdout/stderr, exit 0 on every runtime failure (panics included); clap argv errors keep clap's behavior. Testable core with injected tmux/attribution effects; unit tests with a fake adapter.
5. Tests: CLI smoke (SessionStart entry with TMUX_PANE pane, spool written; Stop event touches no spool/socket/tmux), failure paths (corrupt, locked, read-only store, invalid JSON, empty stdin, missing tmux), hung daemon, sentinel privacy through the binary, timing median over 50 runs with a loose bound. Traced capability scenario for `hook write` (daemonless, with daemon, and a state event asserting no exec/socket).
6. Docs: capabilities.toml comments/ADR citations, operations.md (silent writer, socket bound), CHANGELOG; verify tui --snapshot frames unchanged; checks.sh and run-tests.sh; finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions (defaults taken where the story left a choice):
- Trait seam: `live_events_from_payload(payload, context, observed_ms)`, `live_snapshot_from_payload(...)`, `live_reducer()`, `is_attribution_event(payload)` on `HarnessAdapter`; `observed_ms` is a parameter because `LiveContext` carries no time. Claude maps only `SessionStart` (the `compact` source flags `after_compaction`) through a borrowed serde view (`LiveHook<'a>`); a payload with no `hook_event_name` is still attributed, a non-string one is not.
- pid: the harness's exported pid (`CLAUDE_PID`) when it names a running process, else the /proc walk; both the live entry and the attribution record use it. No comm check, since an npm-installed Claude runs as `node`.
- tmux: pane from `TMUX_PANE`, socket from `$TMUX`; `TMUX_PANE` without `$TMUX` is ignored. tmux is asked only when the store has no entry for the session, and a session start's attribution answer is reused, so a session start costs the same two `display-message` calls as before.
- `--state-root PATH` now holds the live store too, so experiments never touch the real store.
- Socket bound: 2 s per read and write, hook ingest only; status/refresh/snapshot stay unbounded (refresh can rebuild the graph) and are filed as CSP-836.
- clap argv errors keep clap's usage message and exit 2 (documented in ADR 0204 and operations.md); every runtime failure, panics included, is silent with exit 0.

Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) pass; run-tests.sh exit 0, 5067 passed, 0 failed, 1 ignored; `just tuikit-features` exit 0; traced capability test passes with the new hook write scenarios (strace present); `tui --snapshot` frames for 9 views byte-identical before and after (frames.sh).
Timing: in-test debug median about 9 ms for both a session start and a state event (50 runs each, bound 100 ms). Manual release medians over 50 runs through a `printf | env -i` pipeline: session start 8.6 ms, state event 5.5 ms, PostToolUse with a 1 MB tool_response 24.0 ms, against 4.8 / 5.2 / 24.2 ms for the same pipeline into `sh -c 'cat'`, so the writer itself adds about 4 ms to a session start and well under 1 ms to a state event.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus hook write` is now a fast, silent live-state writer. `HarnessAdapter` gained `live_events_from_payload`, `live_snapshot_from_payload`, `live_reducer`, and `is_attribution_event` (defaults keep every other harness as it was); Claude Code maps `SessionStart` to `SessionStarted` through a borrowed whitelisted serde view and attributes only `SessionStart`. The writer (now `src/cli/hook_write.rs`) applies live events to the store before the attribution path, takes the pane from `TMUX_PANE` and the socket from `$TMUX`, asks tmux only on an entry's first sight, uses `CLAUDE_PID` else the /proc walk plus the pid's start time, and is silent with exit 0 on every runtime failure, panics included (clap argv errors keep exit 2, documented). `client_hook_ingest` is bounded by `HOOK_INGEST_TIMEOUT` (2 s per read and write) and falls back to the spool.

Verified by: unit tests with a fake adapter (first-sight tmux, ordering, background, busy store), Claude mapping and privacy tests, socket tests with listeners that never accept or never answer, `tests/hook_write.rs` (session start, state event touches no socket/tmux/spool, one test per failure path, stuck daemon, sentinel privacy through the binary, timing), and new traced capability scenarios asserting a state event starts no process and opens no socket. checks.sh, run-tests.sh (5067 passed, 0 failed), and `just tuikit-features` pass; `tui --snapshot` frames are byte-identical. Debug median about 9 ms per event in the test; release about 4 ms over a bare process spawn for a session start. Follow-up: CSP-836 (bound the other socket calls).
<!-- SECTION:FINAL_SUMMARY:END -->
