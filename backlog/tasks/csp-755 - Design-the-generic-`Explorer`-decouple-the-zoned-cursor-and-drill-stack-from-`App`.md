---
id: CSP-755
title: >-
  Design the generic `Explorer`: decouple the zoned cursor and drill stack from
  `App`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 08:24'
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
- [x] #1 A design document gives the `ExplorerSource` trait, the outcome type, the state split, and the list of `App` fields and where each goes
- [x] #2 The migration is cut into steps that each leave `main` releasable
- [x] #3 The document gives a go or no-go with the reasons
- [x] #4 The right-pane keys that mean different things by focus (`F`, Enter, Backspace) are listed with the table that will declare them
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Re-measure the explorer code by responsibility (explorer.rs, app/explorer_nav.rs, the ExplorerState block and explorer methods in app.rs, ui/right_panel.rs, the keymap, runtime, and snapshot branches) and list every App field, Msg variant, and Action variant the state machine touches.
2. Probe behavior the design depends on, with throwaway checks that are not committed: snapshot frames of drilling, going back, armed back, and the right pane's Esc, h, and PgDn on the showcase fixture; a reducer test of a refresh while drilled.
3. Design the generic explorer: ExplorerModel and ExplorerSource traits, the state (cursor by key, reseat rule, folds, full view, drill frames, armed back, window), the outcome type and its mapping onto property_grid intents and the overlay conventions, Breadcrumb with the elision rule shared with TabBar, StackedMasterDetail, and the right pane's single key table.
4. Write docs/plans/tui-framework/explorer-design.md: measured breakdown, traits and state in Rust, outcome type, Breadcrumb and layout, migration steps, risks and test porting, the focus-dependent keys and their table, the destination table for every App field and Msg/Action variant, and a GO or NO-GO.
5. File the defect the probe found and any sub-tasks the migration needs (labels tui-framework, milestone m-25) with dependencies; run backlog doctor.
6. Record the decision in ADR 0188 and the index, add a short paragraph to docs/design.md, link the design from the plan README, and add judgment calls to operator-review.md.
7. git diff --check (docs-only), finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Measured at a5af2807 with a throwaway line counter: explorer.rs 2,109 lines (builders 47.6%, domain about 74%), explorer_nav.rs 355 (about 190 generic), app.rs explorer parts 230, right_panel.rs 496; about 750 of about 3,400 host lines would be replaced (22%). App fields re-counted from the `self.` accesses in explorer_nav.rs, the ExplorerState block, the cursor-row helpers, and search_open.rs.

Probes, not committed: a snapshot binary on the showcase fixture (drill, two drills, a three-hop drill at 40 columns, armed Backspace and its second press, Esc, h, PgDn on the right pane, and Tab then Backspace after a drill, which showed the left pane's Backspace popping the right pane's drill); a reducer test appended to app_tests.rs, run alone with tmux isolated, then reverted with git checkout, which reproduced a refresh discarding a drill into a neighbor with no left-pane row (filed as CSP-789).

Decisions: GO with a stop rule; ExplorerModel (stored types) split from ExplorerSource (per-call data access) so the App field carries no lifetime; outcome type adds Armed and Refused to the eight named variants; EXPLORER_KEYS with an exact Enter so Shift-Enter and Alt-Enter still reveal, and a focus guard for chords the right table refuses. CSP-756 split into CSP-789, CSP-790, CSP-791 (CSP-756 depends on all three); CSP-758 now depends on CSP-790; backlog doctor reports no cycles.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Wrote `docs/plans/tui-framework/explorer-design.md`, the design for the generic `Explorer`, and recorded the decision in ADR 0188. **Verdict: GO**, in seven steps with a stop rule and a written no-go fallback.

The design gives: a measured breakdown of the explorer code at `a5af2807` by file and by responsibility (about 750 of about 3,400 host lines would be replaced, 22%); the `App` fields the state machine uses (the inventory's seven, re-counted after CSP-710, CSP-724, and CSP-750) with each destination; `ExplorerModel` (the stored types) and `ExplorerSource` (zones of rows with stable keys, `follow`, `tag`, `save`, the grid) in Rust; the `Explorer<M>` state and its ten rules (cursor by key, reseat with a per-zone `lands` flag, sections, every-property switch, drill, back, armed back, refresh, window, drawing); the outcome type (the eight named variants, `LeaveRight` as `Leave`, plus `Armed` and `Refused`) mapped onto the property grid's intents and the overlay outcome; `Breadcrumb` on one `text::elide` rule shared with `TabBar` and `PriorityLine`; `StackedMasterDetail` and `SectionRule` with today's arithmetic; the right pane's keys by focus and the table that declares them (`EXPLORER_KEYS`, resolved by the host as `RIGHT_PANE_KEYS`); a 14-row risk list; the test porting plan (explorer_tests.rs: 29 builder tests stay, 7 move or gain framework counterparts; 26 app tests stay as integration tests); and a destination table for every `App` field, `Msg` and `Action` variant, and function. The first host step and the first framework step are stated precisely.

Filed: CSP-789 (a refresh discards a drill into a neighbor with no left-pane row; reproduced by a throwaway reducer test), CSP-790 (`Breadcrumb` and `text::elide`), CSP-791 (`StackedMasterDetail` and `SectionRule`). CSP-756 now depends on all three; CSP-758 depends on CSP-790. Also updated: the ADR index, `docs/design.md` (one paragraph), the plan README (link and story tables), and `operator-review.md` (eight judgment calls, including the proposed key meanings and closing CSP-435).

Verified: the design was checked against the code by listing every `App` field, `Msg` and `Action` variant, and explorer function with its destination (sections 1.6 and 9); behaviors it depends on were checked with throwaway `--snapshot` frames on the showcase fixture and one reducer test, neither committed. Docs-only change: `git diff --check` is clean and the new files have no trailing whitespace; the full suite was not run (no code changed); `backlog doctor` reports no cycles.

**Regressions**
- None: no visible TUI change (design spike).

**Consistency wins**
- None yet; the design lists the ones CSP-789, CSP-790, CSP-791, and CSP-756 will land.
<!-- SECTION:FINAL_SUMMARY:END -->
