---
id: CSP-810
title: 'Make `hook write` a fast, silent live-state writer'
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-809
documentation:
  - docs/adr/0203-the-live-state-store-folds-agent-events-per-session.md
  - >-
    docs/adr/0205-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
  - docs/plans/live-state/research/01-claude-code.md
priority: high
type: enhancement
ordinal: 896000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code and Codex run `conspectus hook write` once per event, and a state event can fire twice per tool call. Today the command waits on the daemon's full resolve and `graph.bin` rewrite, the socket client has no timeout, and a bug could print to stdout (which Claude feeds to the model) or exit 2 (which blocks a tool). The live-state path has to be cheap and unable to disturb the agent.

Scope: Add a `HarnessAdapter` method that maps a payload to a neutral `LiveEvent` (the sibling of `hook_record_from_payload`), parsing payloads in memory against a typed whitelist (ADR 0205) so large `tool_response` bodies cost nothing to skip. Route `hook write` through the store: read `TMUX_PANE` directly, spawn tmux only on first sight of an entry, write the store before the existing attribution path, never open a socket for non-session-start events, print nothing on stdout or stderr, and exit 0 on every error. Add a read and write timeout to the daemon socket client so a stuck daemon cannot hang a session start. Add the `hook write` path to the traced capability test and update `docs/capabilities.toml` and ADR citations if needed.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: A timing test (median under 15 ms per event on the test machine; document the number, do not make CI flaky), stdout and stderr empty on success and on forced failures, exit code 0 on a corrupt store, a locked store, a missing tmux, and invalid JSON, a hung-daemon test for the new socket timeout, and the sentinel privacy test through the full command.

Blockers: The store.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `hook write` for a state event touches no socket and spawns no process after an entry's first sight
- [ ] #2 `hook write` prints nothing and exits 0 on every error path, with a test per path
- [ ] #3 The daemon socket client times out instead of hanging
- [ ] #4 The capability tests cover `hook write`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
