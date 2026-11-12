---
id: CSP-813
title: Expose live state in CLI output
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:27'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies:
  - CSP-812
  - CSP-811
documentation:
  - >-
    docs/adr/0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
priority: medium
type: enhancement
ordinal: 899000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Live state should be reachable from scripts and from `conspectus` commands before any TUI work, both to prove the data layer and because the machine-readable graph is a design requirement. The first slice also needs a way to see why a harness shows nothing.

Scope: `graph --format json` carries `node_live` (done by the side table; add a golden with state). `conspectus table` gets a `live` column for sessions and a counts column for muxes, off by default like `activity`, listed by `conspectus columns`. `node show` prints state, detail, since, age, quiet or stale, and source. `hook status` gains a health line per harness: events seen recently, or "active but no events: hooks may be untrusted, disabled, or not installed". The `--state` filter is left for the filtering story.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Table and node-show goldens with a populated and an empty store, the `columns` listing test, health line for each of installed/active/silent.

Blockers: Binding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `conspectus table --columns ...,live` shows the effective state with its detail and age
- [ ] #2 `node show` on a session and on a mux prints the live fields
- [ ] #3 `hook status` reports a health line per harness
- [ ] #4 Goldens cover populated and empty stores
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
