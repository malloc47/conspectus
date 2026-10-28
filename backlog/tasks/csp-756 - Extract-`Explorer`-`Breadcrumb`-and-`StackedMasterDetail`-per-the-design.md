---
id: CSP-756
title: 'Extract `Explorer`, `Breadcrumb`, and `StackedMasterDetail` per the design'
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-755
  - CSP-729
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 842000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
This is the last and riskiest extraction, gated by the design spike. The explorer is the part of the TUI most entangled with the graph model, and extracting its generic core completes the property-grid story by giving the right pane a reusable host (inventory slice 07).

Scope: Implement the design: a generic zoned cursor with stable row keys and reseating, the drill stack with an armed back, an outcome-typed key handler, `Breadcrumb` with elision, and `StackedMasterDetail` layout (content-sized list zone, divider, remainder with a minimum detail height). The node builders and the `impl App` wiring that adapt Conspectus stay behind. If the spike said no-go, close this task with that decision instead.

TUI changes:
- Consistency wins: the right pane's keys are declared in one table; the breadcrumb elides instead of clipping; the explorer is testable without an `App`
- Regressions or behavior changes: `h`, `l`, Page keys, and Esc in the right pane may gain meanings that the design chooses, each listed

Tests and checks: The explorer tests (`explorer_tests.rs`, 1,690 lines) are ported to the generic core and the adapter. `--snapshot` of drilling in, going back, and armed back on the showcase fixture.

Blockers: The design spike and the overlay contract.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The generic explorer core, breadcrumb, and stacked master-detail layout are in the framework as the design specifies, or the task is closed on the design's no-go with the reason recorded
- [ ] #2 The explorer is driven through an outcome type, not `Msg` variants plus runtime branching
- [ ] #3 The right pane's key meanings are declared in one table
- [ ] #4 Explorer tests are ported and none is dropped
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
