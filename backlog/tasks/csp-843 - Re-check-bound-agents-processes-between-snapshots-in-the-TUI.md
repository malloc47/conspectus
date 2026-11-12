---
id: CSP-843
title: Re-check bound agents' processes between snapshots in the TUI
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 09:05'
updated_date: '2026-11-12 13:46'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-814
priority: low
ordinal: 928000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI's live overlay checks each bound entry's pid (and start time) only when it binds: after a rewrite of the live-state store, or when a new snapshot lands. An agent killed without a goodbye (SIGKILL, a closed terminal) writes nothing to the store, so its rows keep its last state (`working`, `waiting`) until the next snapshot arrives, at the refresh interval (30 s by default, with or without a daemon), and only then read `exited/killed`. CSP-814 chose not to touch `/proc` on ticks where the store is unchanged.

A periodic re-check of the bound pids alone (a stat of `/proc/<pid>` and its `stat` file per bound entry every few seconds, no store read) would show a killed agent within that period. The cost and the period are the choice to make; the time rules of ADR 0203 already cover processes that cannot be checked.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A killed agent's rows read exited/killed within the chosen period with no store rewrite and no new snapshot
- [x] #2 Ticks between re-checks read neither the store nor /proc
- [x] #3 The period and its cost are recorded in the plan or an ADR
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `LiveOverlay::recheck(now)` in `src/tui/live_overlay.rs`: at most once per `PROCESS_CHECK_INTERVAL` (1 s, counting the check each bind makes), re-probe the pid of each bound entry whose process may still run (not a recorded exit, not already found dead) through the overlay's `PidProbe`, and update its `liveness` in place; nothing when no entry is bound; no store read.
2. `App::poll_live` calls it each tick beside `poll` and `aged`, and a change refills the rows in place through the existing `refill_live` path (no tree rebuild).
3. `ProcProbe::status` reads `/proc/<pid>/stat` first, so a running process costs one read (a stat of the pid dir and of `/proc` only when the read fails); same answers as before.
4. Tests with the `Pids` fake (kill, pid reuse) and explicit `Instant`s: killed and reused pids read exited/killed within the period; ticks in between check nothing and read no store; binds restart the period; exited, dead, and pid-less entries are not re-checked; an App-level test shows the row cell changes with no tree rebuild.
5. Record the period and cost in ADR 0205 decision 4, the plan README (section 3.1 and the cost table), and docs/design.md; verify frames are byte-identical.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Period: 1 s (`PROCESS_CHECK_INTERVAL`), the same as the store re-read and age check, counting the probe every bind makes, so a bind followed by a recheck in the same second probes once. The TUI tick is 100 ms, so nine of ten ticks check nothing.
Cost: one `PidProbe::liveness` per bound entry whose process may still run; `ProcProbe` now reads `/proc/<pid>/stat` first, so that is one read per live process (a failed read adds a stat of the pid dir and of `/proc`, once, since a dead entry is not re-checked). Entries with a recorded exit, already found dead, or with no pid cost nothing.
Choice: skipping already-dead entries means a pid with no recorded start time that the kernel reuses does not flip back to alive between binds; the next bind re-probes as before.
Validation: `run-some.sh --lib -- tui::live_overlay live_store::process tui::consistency_tests::live` (27 passed); `checks.sh` exit 0; `run-tests.sh` exit 0 (5377 passed, 0 failed, 1 ignored); frames.sh: all nine frames byte-identical to the pre-CSP-820 base.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The TUI's live overlay now re-checks the processes of its bound entries between binds, at most once a second (`PROCESS_CHECK_INTERVAL`, counting the check each bind makes). A pid found gone, or handed to a process with another start time, sets the binding's liveness to dead, and `App::poll_live` refills the row cells in place through the existing path (no tree rebuild), so a killed agent reads `exited/killed` within a second with no store rewrite and no new snapshot.

Cost: one read of `/proc/<pid>/stat` per bound entry whose process may still run (`ProcProbe::status` now reads `stat` first); no store read, no process started, nothing when no entry is bound; entries with a recorded exit or already found gone are not re-checked. Recorded in ADR 0205 decision 4, the plan README (section 3.1 and the cost table), and docs/design.md.

Tests (probe fake, explicit instants): killed and reused pids read exited/killed at the period; ticks in between read neither store nor process; binds restart the period; exited, dead, and pid-less entries are skipped; an App-level test shows the cell change with `tree_rebuilds` and the selection unchanged. Verified with `checks.sh` and the full `run-tests.sh` (5377 passed, 0 failed); frames.sh frames byte-identical.
<!-- SECTION:FINAL_SUMMARY:END -->
