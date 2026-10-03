---
id: CSP-307
title: 'Add filter, grouping, and sort controls to dev scenario exploration'
status: Done
assignee: []
created_date: '2026-05-31 04:15'
labels:
  - test
milestone: m-11
dependencies: []
ordinal: 279000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: make `conspectus dev scenario tui <name>` accept the same
  pure exploration flags as normal `conspectus tui`: `--view`,
  `--grouping`, `--harness`, `--mux-state`, `--max-age`, and
  `--sort`. In the static scenario TUI, enable the controls overlay,
  grouping cycle, clear-filters action, and sort/filter changes by
  rebuilding row trees from the pre-materialized scenario snapshot
  instead of running live discovery. Keep mutating or host-affecting
  actions disabled (`attach`, `resume`, rename writes).
- Tests: CLI smoke tests for scenario TUI flag validation and
  scenario table/filter output where practical; reducer/runtime tests
  for static controls applying filter/grouping/sort without invoking
  live discovery. Update `docs/dev-scenarios.md` with examples.
- Manual checks: run `cargo run -- dev scenario tui ambiguous-mux
  --grouping none --mux-state ambiguous --sort recency` and verify
  the TUI opens on the filtered static scenario graph; use the
  controls overlay to change filters/grouping and confirm rows
  rebuild in place.
- Blockers: none for filter/grouping; visible sort behavior may need
  follow-up if a row-tree builder does not yet consume `Sort`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus dev scenario tui` now accepts filter,
grouping, and sort flags before launch. The static scenario TUI
enables controls overlay, grouping cycle, clear-filters, and
view-switch rebuilds against the pre-materialized scenario
snapshot, while attach/resume/rename remain disabled. Added CLI
smoke tests for the hidden TUI flag surface and validation, and
updated `docs/dev-scenarios.md` with filter/group/sort examples.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-007`
