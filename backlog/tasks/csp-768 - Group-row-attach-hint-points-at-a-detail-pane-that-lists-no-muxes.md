---
id: CSP-768
title: Group-row attach hint points at a detail pane that lists no muxes
status: To Do
assignee: []
created_date: '2026-10-28 04:37'
labels:
  - tui
dependencies: []
priority: low
ordinal: 854000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pressing `a` on a workspace, repo, or checkout row whose sessions are ambiguous about two or more muxes says "attach: N ambiguous muxes here — pick one in the detail pane". The right pane no longer lists them: the sectioned detail model that emitted an `Ambiguous muxes` section was replaced by the graph explorer (and deleted by CSP-710), and a group node has no explorer link to those muxes. The operator is told to pick from a list that does not exist.

Decide the intended path: either surface the group's ambiguous muxes in the explorer (for example as rows the operator can drill into or attach from), or reword the hint to send the operator to a session or mux row. `ambiguous_muxes_for_group` in `src/tui/rows/mod.rs` already computes the set. ADR 0071 decision 2 is marked superseded; a new ADR is needed if the explorer gains the rows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The hint on a group row with several ambiguous muxes names a place that shows them, or the explorer shows them
- [ ] #2 Tests cover the chosen behavior for workspace, repo, and checkout rows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
