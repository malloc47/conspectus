---
id: CSP-836
title: Bound the daemon socket calls the TUI and CLI make
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 04:32'
updated_date: '2026-11-13 18:58'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
ordinal: 921000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook write`'s `hook_ingest` call now times out after 2 s on each read and write, but `status`, `refresh`, and `snapshot` still wait indefinitely on the daemon socket. A daemon that accepts a connection and never answers (stuck on its writer lock, or wedged) hangs `conspectus status`, a CLI command that asks the daemon for its snapshot, and the TUI's snapshot fetch. A `refresh` can legitimately take as long as a full rebuild, so one bound does not fit every command; each needs a bound that fits it, or a way for the caller to give up and fall back to local discovery.

Context: `call_socket` in `src/server/mod.rs` already takes a per-call timeout; `call_command` passes none. Surfaced while making `hook write` silent and bounded.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A daemon that accepts and never answers no longer hangs the snapshot fetch of the TUI or a one-shot command; each falls back as it does when no daemon runs
- [x] #2 `conspectus status` and `refresh` either finish or report a timeout within a documented bound
- [x] #3 Tests use a listener that never replies
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Give every socket call a bound that fits the command (src/server/mod.rs): `status` and `snapshot` answer from memory, 5 s; a refresh of the mux or harness class, 15 s (it may wait for the writer lock behind another cycle); a git, forge, or full refresh, 2 minutes (forge fetch plus rebuild). `hook_ingest` keeps 2 s. The bound applies to each read and write, as the ingest bound does; no call waits indefinitely any more.
2. Add a `ClientOutcome::TimedOut` variant so callers can tell a silent daemon from a broken connection.
3. Callers: the one-shot commands' and the TUI's snapshot fetch fall back to local discovery as with no daemon (unchanged let-else); `status` reports the timeout and exits non-zero; `refresh` reports the timeout and exits non-zero without a local rebuild (the daemon may still be refreshing; a second rebuild would race its graph.bin write and fetch the forges twice); the TUI's refresh nudge warns and shows a local rebuild, as on any other socket error.
4. Tests: unit tests with listeners that never reply (timed-out variant, the bound per class); integration tests in tests/cli_serve.rs with a listener that accepts and never answers: a one-shot command and `status` and `refresh --class mux`.
5. Docs: docs/operations.md (the bounds), docs/design.md (transport paragraph), CHANGELOG.md, plan README section 8.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented per the plan, recorded as ADR 0219. Bounds: snapshot and status 5 s, mux/harness refresh 15 s, git/forge/full refresh 2 min, hook_ingest 2 s, each per read and write. New ClientOutcome::TimedOut. A timed-out refresh exits 1 without a local rebuild (the daemon may still finish). connect itself stays unbounded; only a full listener backlog would block it.
Validation: run-some.sh --lib server:: (32 passed); run-some.sh --test cli_serve never_answers (4 passed: graph --format json and tui --snapshot fall back after 5 s, status and refresh --class mux report the timeout after 5 s and 15 s, no graph.bin written by the refresh); checks.sh exit 0; run-tests.sh exit 0, 5,627 passed, 0 failed, 1 ignored (baseline 5,620).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every daemon socket call now has a bound that fits its request (ADR 0219): `snapshot` and `status` 5 s, a `mux` or `harness` refresh 15 s, a git, forge, or whole-graph refresh 2 minutes, `hook_ingest` 2 s as before, each on every read and write. A new `ClientOutcome::TimedOut` tells a silent daemon from a broken connection. The one-shot commands' and the TUI's snapshot fetch fall back to a local rebuild silently; `status` and `refresh` report the timeout and exit 1, and a timed-out `refresh` does not race the daemon with a local rebuild; the TUI's refresh nudge logs a warning and shows a local rebuild. Verified with unit tests on listeners that never reply and integration tests in `tests/cli_serve.rs` against a daemon that accepts and never answers (`graph`, `tui --snapshot`, `status`, `refresh --class mux`), checks.sh, and the full suite (5,627 passed). Documented in docs/operations.md (bounds table), docs/design.md, CHANGELOG.md, and the plan README.
<!-- SECTION:FINAL_SUMMARY:END -->
