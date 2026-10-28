---
id: CSP-713
title: Reproduce and fix Enter-to-copy and `o` on expanded node fields
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: bug
ordinal: 799000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enter-to-copy and `o` (inspect the full value) index `core_fields` while the rendered rows index the expanded field list, so with `F` on, the extra fields do nothing. No test covers it. The inventory found this by reading (slice 07, section 3.17). The future `PropertyGrid` copy and inspect outcomes depend on the right index, so the defect is settled before that extraction.

Scope: Write a failing test that expands the field list and copies or inspects an extra field. Fix the indexing in the explorer. If it does not reproduce, record that in the inventory file and close this task.

TUI changes:
- Consistency wins: copy and inspect work on every field the pane shows
- Regressions or behavior changes: none expected

Tests and checks: A regression test covers `F` on and off for copy and inspect. Manual check with `conspectus tui --snapshot --snapshot-keys` on the showcase fixture.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The defect is reproduced by a failing test, or recorded as not reproducible in the inventory file
- [ ] #2 With `F` on, Enter-to-copy and `o` act on the field under the cursor for every shown field
- [ ] #3 A test covers copy and inspect with the field list expanded and collapsed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
