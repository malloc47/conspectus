---
id: CSP-738
title: Collapse the pin edit form onto `Form<M>` with an override row
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 08:50'
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
- [x] #1 The pin edit form is a `Form<M>` instance using an `OverrideRow`
- [x] #2 Ctrl-R is a per-row binding declared in the form's key table
- [x] #3 The decision on keeping both pin rename UIs is recorded with a reason
- [x] #4 Every existing pin edit test passes or is updated with a note
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Framework: add `tuikit::form::OverrideRow` (inherited value, source word, overridden-at-open flag, hint closure over an `OverrideView`) with a `Standing` (Follows, Kept, Changed, Reset); `Field::with_override`, `Field::hint`, `Field::standing`, `Values::standing`; `FormKey::Inherit` resets the focused override row only (the host's table binds the key). A form with override rows ends every text/path/choice row in one right-aligned source column (`Cols::source`, `source_cells`). `Form::intro` (fixed paragraph above the rows) and `Form::details(n)` (fixed band of the focused field's hints under the rows). Tests in `form/override_row_tests.rs` and an `override.golden`; reference page section in `docs/controls/form.md`.
2. Host: rebuild `pin_edit.rs` as `PinEditForm` holding a `Form<Msg>` (MuxLaunchForm precedent): summary note, pin name (rename, `validate_id` and collision check as its field check), live-mux note, label, the kind's fields as override rows (cwd/location/path via PathField, harness Choice with custom, argv with the shared launch_argv helpers and a strict check, options CheckGroup kept in step after each key), intro paragraph, two detail lines, fixed footer. Field map by kind, labels in the shared vocabulary, layer words, hints, and request -> `Msg::CommitPinEdit` (set/unset) stay in the host. Retire the host row helpers (`widgets/form.rs`) only the old form used; keep their theme tests against the framework rows.
3. Update pin_edit_tests and consistency tests deliberately; decide to keep both rename UIs (record in operator-review.md); file a follow-up for aligning the prompt's name check.
4. Capture after frames (/tmp/tuifw-baseline/frames-738-after) and compare by category; ADR for the override row and the pin edit form; docs/operations.md and CHANGELOG if operator-visible.
5. fmt, clippy, full isolated suite, rustdoc, git diff --check; final summary with Regressions and Consistency wins.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework: `OverrideRow` (inherited value, source word, overridden-at-open, hint closure over `OverrideView`), `Standing` (Follows/Kept/Changed/Reset), `Field::with_override`/`hint`/`standing`/`hints`, `Values::standing`, `Cols::source` with `source_cells`, `Form::intro` and `Form::details(n)`; `FormKey::Inherit` resets the focused override row only, and only where the form's own table binds it. Detail lines are drawn full width so the selection convention holds (a conformance case covers it). Golden `form/goldens/override.golden`.

Host: `PinEditForm` holds a `Form<Msg>` (MuxLaunchForm precedent) and keeps options/argv in step after each key with `launch_argv`. A field without an override follows its value in effect (source = effective layer); with an override it follows the no-override resolution, as before. `widgets/form.rs` deleted (single consumer); its theme tests draw tuikit rows from `widgets/form_tests.rs`.

Tests updated deliberately: pin_edit_tests `it_lists_the_pin_name...` and `up_and_down...` (label `branch`), `the_skip_permissions_option...` (the options row is a focusable field, `focus() == Some(OPTIONS)`, was `focused_label() == None`); consistency `forms::editing_a_pin_lists_every_field...` and ui_tests `opening_edit_builds_the_form...` (labels through `label_for`); ui_tests `form_rows_with_wide_values_fit_the_form` (framework row directly); tuikit `ctrl_r_reaches_no_field` renamed `the_reset_key_leaves_a_field_without_an_override_alone`. Seven new host tests, sixteen new framework tests.

Frames: /tmp/tuifw-baseline/frames-738-{before,after} (mux live, planned mux with overrides, session, checkout, PR; open, edited, reset, kept, options, refused rename, refused argv) at 100x30, 60x20, 40x10; compared with compare-738.py. No repo pin in the showcase.

Checks: fmt, clippy -D warnings, full isolated suite 4121 passed 0 failed 1 ignored, rustdoc -D warnings, git diff --check. Follow-up filed: CSP-778 (rename prompt takes the form's name checks; by reading, a live mux pin renamed to a taken name through the prompt renames tmux before the store refuses).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tuikit::form` gained `OverrideRow`: a text, path, or choice field follows an inherited value until edited, the reset key (`FormKey::Inherit`, bound only in the form's own table) puts that value back, and every row ends in one source column (the source word, `kept`, or `will keep`/`will follow` in `warning`). `Values::standing` tells `on_submit` what to keep or drop. `Form::intro` adds a fixed paragraph above the rows and `Form::details(n)` a fixed band of the focused field's hints. The pin edit form is `PinEditForm`, a `Form<Msg>` host like the launch form. It has the summary note, `pin name` (with `validate_id` and the collision check), the live-mux note, the label, and the kind's fields as override rows: `PathField` directories, a harness `Choice`, an argv checked as `pin edit --set` reads it, and an options `CheckGroup` kept in step through `launch_argv`. The intro and two detail lines explain following. The host keeps the field map, labels, layer words, hints, and the `Msg::CommitPinEdit` request (set/unset, saved as `StoreOp::SavePin`). Both rename UIs stay (ADR 0170 decision 7, operator-review.md); CSP-778 gives the prompt the form's checks. The host row helpers in `widgets/form.rs` are gone. ADR 0170; `form.md` gained "Override rows".

Verified: 16 new tuikit tests (standing, reset, the form-owned key, source column, detail band, intro, golden `override.golden`, a conformance case). Host: the 24 pin edit tests pass, 3 updated with notes (labels, options focus), 7 new; consistency and ui tests updated for labels. `conspectus tui --snapshot` before/after frames at 100x30, 60x20, 40x10 for live and planned mux, session, checkout, and PR pins: open, edited, reset, kept, options, refused rename and argv (/tmp/tuifw-baseline/frames-738-*). The showcase has no repo pin. fmt, clippy -D warnings, the full isolated suite (4121 passed, 0 failed, 1 ignored), rustdoc -D warnings, and `git diff --check` pass.

**Regressions**
- `Ctrl-R` on the pin name row no longer puts back the name the form opened with.
- The `pin  name · kind · status` note sits under the intro and scrolls with the rows. It used to be fixed at the top.
- At 40x10 the two detail lines no longer fit; the whole footer shows instead.
- Labels follow the shared vocabulary: `worktree_branch` reads `branch`, `resume_into` reads `resume in`. They no longer match the `pin edit --set` names.
- The path hint already followed the one rule; only its width changed with the wider value column.

**Consistency wins**
- The override mechanism is a reusable form row, and `Ctrl-R` is a binding of this form's table, not a global.
- A refused save puts its reason on the error line directly above the keys, flush left and cut with `…`, as in the other forms. It used to sit indented under the detail lines. The focus moves to the field the refusal is about.
- The label column narrows from 15 to 11 cells for mux pins, so values get four more cells (another harness chip; a long argv fits).
- Resetting a field with no override shows its source word again (it said `will follow`) and puts back its value in effect.
- A kept value with nothing to follow says `Ctrl-R drops it; there is nothing to follow yet`. It used to say "Ctrl-R goes back to nothing to follow yet; type a value…".
- At 60x20 one more field row shows when the cursor is low (the summary scrolls).
<!-- SECTION:FINAL_SUMMARY:END -->
