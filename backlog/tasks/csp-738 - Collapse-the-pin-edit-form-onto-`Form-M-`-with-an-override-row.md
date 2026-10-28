---
id: CSP-738
title: Collapse the pin edit form onto `Form<M>` with an override row
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-736
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
priority: medium
type: enhancement
ordinal: 824000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The pin edit form (850 lines) lists every field a pin can change with its current value and where that value comes from; editing a row makes an override and Ctrl-R drops it so the field follows the entity again. The Ctrl-R key means reset-to-inherited here and earlier-search in the search overlay, so it must be a per-component binding, not a global. Pin rename has two UIs and two store paths with different validation (inventory slice 03).

Scope: Add an `OverrideRow` decorator to `Form<M>` (inherited value, source word, hint, reset binding) and rebuild the pin edit form on it with read-only note rows, detail lines, and a check group. The field map by pin kind, the layer words, and the request-to-store mapping stay in Conspectus. Decide whether the form row and the Rename prompt both stay; keep both unless unifying them is trivial, and record the reason.

TUI changes:
- Consistency wins: the override mechanism is a reusable form row; consistent error and footer behavior with the other forms
- Regressions or behavior changes: the path hint follows the one rule chosen for forms; labels follow the shared vocabulary

Tests and checks: The existing `pin_edit_tests.rs` cases pass or are updated with a note. `--snapshot` of a pin with and without overrides.

Blockers: The `Form` kit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The pin edit form is a `Form<M>` instance using an `OverrideRow`
- [ ] #2 Ctrl-R is a per-row binding declared in the form's key table
- [ ] #3 The decision on keeping both pin rename UIs is recorded with a reason
- [ ] #4 Every existing pin edit test passes or is updated with a note
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
