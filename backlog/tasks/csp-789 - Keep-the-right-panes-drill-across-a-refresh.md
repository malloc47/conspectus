---
id: CSP-789
title: Keep the right pane's drill across a refresh
status: To Do
assignee: []
created_date: '2026-10-30 08:08'
updated_date: '2026-10-30 08:22'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/explorer-design.md
priority: medium
type: bug
ordinal: 875000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The live TUI refreshes every 30 seconds. On every refresh `App::recompute_explorer` rebuilds the right pane for the node the left pane's selection names, and keeps the pane's state (cursor, `Other` fold, Expanded Node Detail, breadcrumb) only when that node is the one the pane is focused on. After `Enter` drills into a neighbor that has no row in the active view (a session's mux, checkout, repo, or process from the Sessions view), the left selection still names the session, so the next refresh throws the drill away: the pane jumps back to the session with an empty breadcrumb. A throwaway reducer probe written for the explorer design (CSP-755) reproduced it: drill from `app_for_explorer` into the mux, deliver `SetData` with the same snapshot, and the pane is on the session at depth 0. Separately, no test covers `Backspace` to a frame whose node has gone (`explorer_back` drops that hop with a warning and the next press goes on), which the framework's back rule will change.

The generic explorer's refresh rule (docs/plans/tui-framework/explorer-design.md, section 2.5) reloads the frame the pane shows and keeps the drill while the left selection is unchanged, so the defect is fixed here first, ahead of the extraction (ADR 0151: a defect is fixed before the extraction that would otherwise paper over it).

Scope: Conspectus only. The right pane remembers which left-pane selection it reflects (set when it opens, after the mirror moves the left pane, and after Backspace restores it); a refresh or projection change that leaves that selection alone reloads the focused node in place and keeps the breadcrumb, fold, detail toggle, and cursor; any other selection change opens the pane on the new node as today. When the focused node is gone after a refresh, the pane opens on the left selection's node. A test pins Backspace to a gone node as it behaves today.

TUI changes:
- Consistency wins: a drill survives the periodic refresh, a projection change, and `r`
- Regressions or behavior changes: none expected besides the fix

Tests and checks: reducer tests for each criterion; `conspectus tui --snapshot` cannot show it (snapshot mode's `r` runs real discovery), so the reducer tests are the check.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A reducer test drills into a neighbor that has no row in the active view, delivers a refresh with the same snapshot, and finds the pane still on the neighbor with its breadcrumb, fold, detail toggle, and cursor; it fails before the fix
- [ ] #2 Selecting another row in the left pane still opens the pane on that row's node with an empty breadcrumb
- [ ] #3 A refresh that removes the focused node opens the pane on the left selection's node
- [ ] #4 A test pins Backspace to a node that is gone: it says so, and the next Backspace goes on from the frames that remain
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
