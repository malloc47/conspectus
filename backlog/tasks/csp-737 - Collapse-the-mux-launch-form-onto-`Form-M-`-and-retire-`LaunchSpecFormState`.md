---
id: CSP-737
title: Collapse the mux launch form onto `Form<M>` and retire `LaunchSpecFormState`
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
  - docs/adr/0097-launch-spec-form-primitive.md
  - docs/adr/0096-ephemeral-harness-mux-launch.md
priority: medium
type: enhancement
ordinal: 823000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`MuxLaunchFormState` (717 lines) sits on `LaunchSpecFormState` (457 lines), whose only production consumer it is; the second consumer ADR 0097 relied on, `PinCreateState`, no longer exists. The form has 11 field stops, a Tab-only socket checkbox, a derived command row, and two argv tokenizers that disagree on quotes and escapes, with the pin edit form using both. Harness cycling and option toggling are copied between `launch_spec_form.rs` and `pin_edit.rs` (inventory slice 03).

Scope: Rebuild the mux launch form as a `Form<M>` instance with conditional fields, a derived command row, and a one-of-N pin row. The socket checkbox joins the single tab ring. Dissolve `LaunchSpecFormState` into the form's values. Keep the field list, request builder, and `LaunchDefaults` preview in Conspectus. Reconcile the two argv tokenizers on `argv::split_command_line` and write down the quoting behavior. Mark ADR 0097 superseded in the index.

TUI changes:
- Consistency wins: one ring of tab stops; one argv tokenizer; less duplicated harness and option logic
- Regressions or behavior changes: the Tab-only socket stop becomes a normal row; argv quoting follows one tokenizer, which can change how an edge-case value is split

Tests and checks: The existing `mux_launch_tests.rs` and `consistency_tests/forms.rs` cases pass or are updated with a note. Tokenizer tests for quotes and escapes. `--snapshot` of the form.

Blockers: The `Form` kit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The mux launch form is a `Form<M>` instance and `LaunchSpecFormState` is removed
- [ ] #2 The socket checkbox is a normal row in the tab ring
- [ ] #3 One argv tokenizer is used by the launch form and the pin edit form, with its quoting documented
- [ ] #4 ADR 0097 is marked superseded in the ADR index
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
