---
id: CSP-649
title: Stop forge fetches promptly when the daemon shuts down
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 17:11'
updated_date: '2026-10-21 17:33'
labels:
  - forge
  - server
dependencies: []
priority: medium
type: bug
ordinal: 707000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Ctrl+C on `conspectus serve` sometimes takes seconds to exit. The main thread joins every scheduler thread, and a forge fetch in progress keeps going: Ctrl+C kills the in-flight `gh` calls (same process group), but the fetch then starts its next batches, which run to completion, and the forge thread merges the result under the writer lock. That merge also writes `graph.bin` with the killed calls recorded as failures, and the next daemon seeds from it. Under SIGTERM the in-flight `gh` calls are not signaled at all.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Once shutdown begins, a forge fetch starts no further gh calls, and in-flight system gh calls are killed
- [x] #2 A fetch cut short by shutdown caches nothing; the forge scheduler skips its merge cycle and a refresh in progress answers that the daemon is shutting down instead of publishing
- [x] #3 Output larger than a pipe buffer is still read in full from a gh call that is not cancelled
- [x] #4 The new signal is in docs/capabilities.toml with an ADR, and the capability tests pass
- [x] #5 Tests cover the cancelled batch, the killed child, the skipped merge, and the refused refresh; a live SIGINT and SIGTERM during a fetch exit within about a second
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Two issues found while verifying: a killed shim left a grandchild holding the output pipes, so joining the reader threads waited it out (cancelled calls no longer join them); and the signed-in-hosts lookup () still used the blocking path and held shutdown about 2.4 s (now cancellable, and caches nothing when cut short).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The daemon shutdown latch now cancels forge fetches (ADR 0137): no further batch of `gh` calls starts, in-flight `gh pr list` and `gh auth status` calls are killed (SIGKILL, added to `docs/capabilities.toml`), a cancelled fetch caches nothing, the forge scheduler skips its merge cycle, and a `refresh` whose fetch was cut short answers `shutting_down` instead of publishing. Output is read on threads so large listings can't stall a polled child. Verified with fmt, clippy `-D warnings`, the full nextest suite (2553 passed; new tests for the killed child, the large listing, failure classification, the cancelled batch, the hosts lookup, the skipped merge, and the refused refresh) and an isolated daemon on the operator machine: SIGINT and SIGTERM during a hosts lookup or four in-flight listings exited in 162-206 ms with no orphaned gh, against 2.4-3.3 s before.
<!-- SECTION:FINAL_SUMMARY:END -->
