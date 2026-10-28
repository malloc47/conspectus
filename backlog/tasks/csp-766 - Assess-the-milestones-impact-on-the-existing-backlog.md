---
id: CSP-766
title: Assess the milestone's impact on the existing backlog
status: To Do
assignee: []
created_date: '2026-10-27 20:28'
labels:
  - tui-framework
  - tuifw-close
milestone: m-25
dependencies:
  - CSP-764
  - CSP-762
  - CSP-763
documentation:
  - docs/plans/tui-framework/control-set.md
  - docs/plans/tui-framework/gap-analysis.md
priority: high
type: task
ordinal: 852000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The extraction moves modules, renames concepts, removes views and keys, and changes shared behavior (cursor, close keys, confirmation, theme keys). Many open stories name `src/tui/widgets/*`, `ui/left_panel.rs`, `Controls`, `Picker`, or `LaunchSpecFormState`; some now solve a problem the framework solved; some conflict with a house rule. At planning time there were 124 open stories. Reviewing them once, at the end, with the framework in its final shape, is cheaper and more accurate than guessing now (`docs/plans/tui-framework/control-set.md`, section 11, lists the overlaps already visible).

Scope: List every story that is open at the time of the review and every In Progress story. Classify each as unaffected, needs a path or terminology update, superseded or obsolete, needs re-scoping, or newly blocked or unblocked. Record the result in `docs/plans/tui-framework/backlog-impact.md` and apply it through the `backlog` CLI: update descriptions, dependencies, and labels, and close superseded stories with a final summary that points at the story that solved them. Give an explicit outcome to the known overlaps: CSP-342 (the viewer crate), CSP-390, 391, 392, 393, and 394 (command palette and minibuffer), CSP-430 (`tui-textarea`), CSP-432 (spinners), CSP-434, 455, and 455.01 (path field), CSP-435 (`tui-tree-widget`), CSP-269 and 270 (theme presets and density), CSP-319, 190, 180, and 183 (responsive layout, frame matrix, snapshot coverage), CSP-188 and 191 (labels and density), CSP-616.09, CSP-687, CSP-702, and the m-24 stories. Map each Tier 1 and Tier 2 entry in the gap analysis to an existing story, a new story, or an explicit "not planned". If the runtime evaluation was done, fold its outcome in.

TUI changes: none.

Tests and checks: A final listing of open stories shows every one with a recorded classification. `backlog doctor` passes after the dependency changes.

Blockers: Last in the milestone: it depends on the documentation and separation stories so it sees the final shape.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every story open when the review starts is classified in `docs/plans/tui-framework/backlog-impact.md`
- [ ] #2 Each story needing a change is updated through the CLI, and each superseded story is closed with a final summary that names its replacement
- [ ] #3 Each known overlap named in the scope has an explicit recorded outcome
- [ ] #4 Each Tier 1 and Tier 2 gap-analysis entry maps to an existing story, a new story, or an explicit decision not to plan it
- [ ] #5 `backlog doctor` passes
- [ ] #6 A summary of counts by classification is written in the final summary
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
