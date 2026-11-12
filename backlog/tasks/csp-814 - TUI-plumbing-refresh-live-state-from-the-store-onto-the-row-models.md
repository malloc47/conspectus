---
id: CSP-814
title: 'TUI plumbing: refresh live state from the store onto the row models'
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - >-
    docs/adr/0204-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
  - docs/plans/live-state/research/06-internal-audit.md
priority: medium
type: enhancement
ordinal: 900000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI polls a whole snapshot every 30 seconds and has no push channel, so a hook event would take up to 30 seconds to show. The store is cheap to stat and read, so the TUI can re-bind it between snapshots with or without a daemon. This story builds that path and carries the data onto the row models without rendering anything, so the rendering decisions stay open.

Scope: In the runtime, stat the store once per tick and, when it changed and at least a second has passed, re-read it and re-run `bind_live` against the snapshot the app holds; hold the result as an overlay that the row builders read in place of the snapshot's table. Add a `live` field (effective state, detail, since, confidence, subagent count) to the session row, the mux row (as counts), and the mixed rows, filled for every view by shared code, not rendered. Handle the daemonless and daemon-backed refresh paths, a snapshot swap during an overlay refresh, and a missing store. Nothing reads it for display yet.

TUI changes: none visible: the data is on the row models but no column, glyph, or chip reads it. `conspectus tui --snapshot` frames are byte-identical before and after for the default views.

Tests and checks: Runtime tests for the stat/throttle logic and overlay refresh; consistency tests that the same entity carries the same live data in every view; the snapshot goldens unchanged; a test with a changing store file proving the overlay updates within the interval without a rediscovery.

Blockers: Binding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A change to the store reaches the row models within about a second, with and without a daemon
- [ ] #2 Session, mux, and mixed row models carry the live data, built by shared code for every view
- [ ] #3 TUI snapshot output is unchanged for the default views
- [ ] #4 A missing or corrupt store leaves rows without live data and raises no error
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
