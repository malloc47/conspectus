---
id: CSP-843
title: Re-check bound agents' processes between snapshots in the TUI
status: To Do
assignee: []
created_date: '2026-11-12 09:05'
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
- [ ] #1 A killed agent's rows read exited/killed within the chosen period with no store rewrite and no new snapshot
- [ ] #2 Ticks between re-checks read neither the store nor /proc
- [ ] #3 The period and its cost are recorded in the plan or an ADR
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
