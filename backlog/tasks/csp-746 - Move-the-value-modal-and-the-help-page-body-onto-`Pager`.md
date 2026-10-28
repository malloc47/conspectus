---
id: CSP-746
title: Move the value modal and the help page body onto `Pager`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
  - CSP-712
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: enhancement
ordinal: 832000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The value modal (read-only text with a `y` copy payload) and the help page body are pagers: the modal sets an unclamped `u16::MAX` offset on `G`, and both page by a fixed 8 lines. They sit apart from the viewers' pager although they are the same control in Modal mode (inventory slice 05).

Scope: Re-host both on `Pager` in Modal mode: the value modal as a content adapter with the copy hook, the help body as generated rows. The rebuild-plan `$ ` highlighting stays as a small content hook in Conspectus.

TUI changes:
- Consistency wins: `G` and paging behave like the viewers; one footer and frame
- Regressions or behavior changes: page size changes from 8 lines to the viewport

Tests and checks: Value modal and help tests ported. `--snapshot` of both at 80x24 and 40x10.

Blockers: `Pager`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The value modal and the help body use `Pager` in Modal mode, and their own scroll code is removed
- [ ] #2 The value modal's copy hook still works and is covered by a test
- [ ] #3 `G` lands on the last page and never renders a blank body
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
