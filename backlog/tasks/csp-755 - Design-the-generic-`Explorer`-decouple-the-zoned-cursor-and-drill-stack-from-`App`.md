---
id: CSP-755
title: >-
  Design the generic `Explorer`: decouple the zoned cursor and drill stack from
  `App`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-754
  - CSP-750
references:
  - CSP-435
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: spike
ordinal: 841000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The explorer's control flow is generic (a flat cursor over zones with stable row keys, a drill stack, armed double-Backspace), but about 48% of `explorer.rs` is domain builders, about 13% is domain-free, and the state machine is written as `impl App` using seven `App` fields. Roughly 20 to 25% of the code would move. It works through `Msg` variants plus `Action::ExplorerEnter` branching in the runtime instead of an outcome type. `tui-tree-widget` (CSP-435) does not fit: the explorer is a flat list with one collapsible header, not a tree (inventory slice 07, section 4).

Scope: Write a design (in `docs/plans/tui-framework/`) with an `ExplorerSource` trait, stable row keys and reseating, an outcome type (`Copy`, `Inspect`, `Drill`, `Back`, `LeaveRight`, `Toggle`, `None`), the breadcrumb elision rule, the stacked master-detail layout, and what stays in Conspectus. Include a migration path in steps small enough to land separately, a risk list, and a go or no-go. If the recommendation is no-go, the explorer stays in Conspectus and the extraction story closes with that decision.

TUI changes: none; this is a design spike.

Tests and checks: The design is reviewed against the current `explorer.rs` and `explorer_nav.rs` by listing each `App` field the state machine uses and where it ends up.

Blockers: The property grid and the tree extraction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A design document gives the `ExplorerSource` trait, the outcome type, the state split, and the list of `App` fields and where each goes
- [ ] #2 The migration is cut into steps that each leave `main` releasable
- [ ] #3 The document gives a go or no-go with the reasons
- [ ] #4 The right-pane keys that mean different things by focus (`F`, Enter, Backspace) are listed with the table that will declare them
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
