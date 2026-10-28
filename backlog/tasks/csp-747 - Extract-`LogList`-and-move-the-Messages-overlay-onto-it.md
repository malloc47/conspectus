---
id: CSP-747
title: Extract `LogList` and move the Messages overlay onto it
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
  - CSP-731
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0105-tui-message-log.md
priority: medium
type: enhancement
ordinal: 833000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Messages overlay (`!`) is a scrolling master list of operation outcomes with a detail pane. It sets its own selection highlight (it is one of the three places that read `theme.selection_active`), pages the detail by a fixed 8 lines, and sets an unclamped offset on `G` (inventory slices 04 and 05).

Scope: Create `LogList<R>` with rows `{severity, time, summary}`, a selection, and an embedded `Pager` for the detail. The message payload (`CommandRecord`, `LogTarget`, argv, exit status, stderr, stdout) and the preview banner stay in Conspectus.

TUI changes:
- Consistency wins: the detail pane behaves like every pager; severity colours from the theme
- Regressions or behavior changes: none expected

Tests and checks: Messages tests ported. `--snapshot` at 80x24 and 40x10.

Blockers: `Pager` and the feedback extraction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `LogList<R>` provides the master list and an embedded pager for the detail
- [ ] #2 The Messages overlay is a `LogList` instance whose payload stays in Conspectus
- [ ] #3 The detail pane clamps on `G` and pages by the viewport
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
