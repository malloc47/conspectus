---
id: CSP-594
title: 'Query forges lazily: hourly cadence and a Ctrl-R hard refresh'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 00:56'
updated_date: '2026-10-13 00:57'
labels:
  - p4-fu
milestone: m-9
dependencies: []
priority: high
ordinal: 643000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Forge discovery ran `gh pr list` (two calls per GitHub remote) far more often than the configured interval. Repos with no PRs leave no `github` stamps, so the freshness gate never skipped forge. A five-minute cache keyed by the whole repo set was the only limit, and one new repo missed it. PR previews also re-ran `gh pr view` on every reselect and `r`.

Scope (ADR 0116): fetch forges at startup and then once per `[server.intervals] forge`, default `1h`. Cache listings per remote, stamp PRs with their fetch time, and retry failures within 5 minutes while keeping the last good PRs. Ctrl-R in the TUI is a hard refresh: it drops listings, refreshes the daemon's forge class, and reloads PR previews. PR previews follow the forge cadence.

Tests: listing reuse, per-remote misses, expiry/clear, failure retry, keep-last-good, fetch-time stamps, PR preview reload rules, the Ctrl-R binding, and the PRs-view empty state. Manual: count calls through a `gh` logging shim for the daemon, the TUI on a daemon, and a daemonless TUI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Forge listings are fetched at startup and reused until older than the forge interval (default 1h)
- [x] #2 A newly discovered remote fetches only its own listing
- [x] #3 Ctrl-R refetches forges (locally and through a running daemon) and reloads PR previews
- [x] #4 PR previews are not refetched on reselect or r while younger than the forge interval
- [x] #5 Failed forge reads retry within five minutes and keep the last returned PRs
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0116: forge cadence, per-remote listings, Ctrl-R hard refresh.
2. Replace the forge TtlCache with ForgeListings keyed by remote; forge max age from intervals.forge (default 1h) on the DiscoveryContext.
3. Daemon clears listings on explicit forge/full refresh.
4. TUI: Action::HardRefresh on Ctrl-R, nudge daemon forge class, mark PR previews stale; PR previews follow the forge cadence.
5. Tests, docs, gh-shim validation.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Clearing listings on every scheduled forge tick would double-fetch when the daemon starts, because the first cycles of the other classes have already fetched. Only explicit refreshes clear them. The scheduled tick lands after the listing expires, since the scheduler waits a full interval after the cycle that fetched it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed ADR 0116 (65c3b92), per-remote forge listings with a 1h default interval (d54be54), and the Ctrl-R hard refresh with lazy PR previews (a32fc89).

Verified with `just check` (2309 tests) and by counting calls through a `gh` logging shim on 6 remotes:
- Isolated daemon: 12 calls at startup and none over 25s of harness cycles or explicit harness/mux refreshes. `refresh --class forge` and a full refresh each added 12.
- TUI on that daemon: 0 calls at startup and for `r`; Ctrl-R added 12, showing "Refreshing forges".
- Daemonless TUI: 12 at startup and none over 20s of 2s timer refreshes or for `r`; Ctrl-R added 12.
<!-- SECTION:FINAL_SUMMARY:END -->
