---
id: CSP-647
title: 'Fetch forge listings lazily, outside the daemon writer lock'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 15:52'
updated_date: '2026-10-21 16:19'
labels:
  - forge
  - server
  - tui
dependencies: []
priority: high
type: enhancement
ordinal: 705000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After a tmux detach the TUI nudges the daemon to rescan the mux and harness classes, and the nudge waits on the daemon writer lock. Every daemon cycle runs forge discovery, and when listings are missing (after a daemon restart) or expired (hourly) the cycle that notices runs a full round of `gh pr list` calls, about 9-13 s on the operator machine, while holding that lock. Measured: a mux nudge took 9.95 s during a forge fetch against 0.14-0.23 s otherwise, so the hand-off spinner stays up for the whole fetch. The operator wants forge calls lazy by default, with only an explicit refresh (Ctrl-R, `conspectus refresh`) fetching synchronously.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Daemon mux, harness, and git cycles, and a refresh of those classes, never run gh: they rebuild PRs from cached listings of any age
- [x] #2 The daemon forge class fetches missing or expired listings in the background without holding the writer lock, then merges them in a short locked cycle; a new repo gets its PRs within the forge poll period
- [x] #3 Ctrl-R, conspectus refresh, and refresh --class forge still fetch synchronously for the caller, outside the writer lock
- [x] #4 A daemonless TUI fetches forge listings on a background worker; its timer, hand-off, rename, and launch refreshes never run gh; Ctrl-R fetches
- [x] #5 One-shot CLI commands that show PRs (graph, table prs, node) still fetch
- [x] #6 Tests cover cached-only discovery, prefetch of missing and expired remotes, and the lock not being held while fetching; an ADR amends 0116
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Daemon cycles now publish to ctx.snapshot_path (production value unchanged), so server tests no longer write the real graph.bin. Every forge poll records a status tick so status shows the class alive.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Forge fetches are lazy (ADR 0136). Discovery has a `ForgeFetch` policy; the daemon's mux, harness, and git cycles, refreshes of those classes, and every live-TUI refresh are `CachedOnly` and never run `gh`. The daemon forge class polls every 10 s and fetches missing or expired listings under its own fetch lock, off the writer lock, then merges them in a short cached-only cycle. Ctrl-R, `conspectus refresh`, and `refresh --class forge` still fetch synchronously for the caller, outside the writer lock. A daemonless TUI runs the same fetch on a background worker; one-shot commands still fetch. Verified with fmt, clippy `-D warnings`, the full nextest suite (2545 passed; new tests: cached-only discovery, prefetch of missing/expired remotes only, a fetch completing while the writer lock is held, a mux refresh not waiting on a forge refresh, routine cycles never calling gh, worker fetch policy, background-fetch scheduling), and an isolated daemon on the operator's real repos: nudges stayed at 98-151 ms through its first fetch (93 PRs merged at ~23 s) and at 150-240 ms during an explicit 8.3 s forge refresh, against 9.95 s before.
<!-- SECTION:FINAL_SUMMARY:END -->
