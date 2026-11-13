---
id: CSP-737
title: Collapse the mux launch form onto `Form<M>` and retire `LaunchSpecFormState`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 07:50'
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
- [x] #1 The mux launch form is a `Form<M>` instance and `LaunchSpecFormState` is removed
- [x] #2 The socket checkbox is a normal row in the tab ring
- [x] #3 One argv tokenizer is used by the launch form and the pin edit form, with its quoting documented
- [x] #4 ADR 0097 is marked superseded in the ADR index
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
1. argv: one tokenizer in `src/argv.rs` over one POSIX-style grammar (no expansion; backslash in double quotes escapes only `"` `\` `$` and backtick), as `parse_command_line` (refuses an unclosed quote or a trailing backslash) and `split_command_line` (reads unfinished text as far as it goes); module docs state the grammar; tests for quotes, escapes, and edge cases. Typed argv (the launch form, pin edit, `pin edit --set`) is parsed strictly; text read back from tmux or records stays lenient. `parse_launch_argv` goes.
2. One host helper module for the argv field shared by the launch and pin edit forms: the default an empty field means, cached and recomputed only when the harness or cwd changes (CSP-777), the effective argv, option flags on or off, a typed argv carried across a harness change, and one quoter (`argv::shell_quote`). Pin edit uses it and steps its harness with the framework's `Choice`.
3. Rebuild the launch form as a `tuikit::form::Form<Msg>` wrapped by a host struct that keeps the derived fields in step after each key (`Form::edited`): name, a `server` toggle and conditional `socket` field in the ring, harness `Choice` (custom allowed), options `CheckGroup`, cwd and repo paths, argv, derived `command` row, worktree toggle with conditional branch and repo, one-of-N pin. Field checks carry the old reasons; `on_submit` builds `MuxLaunchRequest`. Retire `LaunchSpecFormState`, its tests, and the form.rs helpers only it used.
4. Update host tests deliberately (mux_launch_tests, consistency_tests forms and text_entry, ui_tests, form_tests, insta snapshots); add the CSP-777 draw-path test for both forms.
5. Docs: ADR 0169 supersedes 0097 (index + 0097 status), operations.md (ring, socket row, argv quoting), CHANGELOG, form.md seam text, operator-review rows.
6. Before/after `--snapshot` frames (default, worktree, socket, error, long values, options, argv error) at 100x30, 60x20, 40x10; full checks; close CSP-777 as covered.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: `6220bed9` one tokenizer in src/argv.rs (strict `parse_command_line`, lenient `split_command_line`, `pin_ops::parse_override` strict); `180ab098` `widgets::launch_argv` shared by both forms (DefaultArgv cache, option flags, carry rule, one quoter), pin edit moved onto it; `b0b2519e` the launch form as a `Form<Msg>` held by `MuxLaunchForm`, `LaunchSpecFormState` and the launch-only form.rs helpers removed, tests and snapshots updated; `4fdb3a59` ADR 0169, index, design, operations, changelog, form control page, operator review.
Validation: fmt, clippy --workspace --all-targets --all-features -D warnings, full isolated suite 4098 passed / 0 failed / 1 ignored, RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps, git diff --check, tuikit independence and documentation tests. Frames: /tmp/tuifw-baseline/frames-737-before and -after (launch form at 100x30, 60x20, 40x10; pin edit frames identical).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The mux launch form is a `tuikit::form::Form<Msg>` held by `MuxLaunchForm` (`src/tui/widgets/mux_launch.rs`), which keeps the fields that follow others in step after each key (`Form::edited`). Fields: `name`; `server`, a toggle (`[ ] own socket`) on the one ring; `socket`, shown while it is on; `harness`, a choice that takes a typed value; `options`, a check group of the harness's launch options; `cwd`; `argv`; `command`, a derived line (a warning when there is no default); `worktree` with `branch` and `repo` shown while on; `pin`, one of three. The checks keep their reasons and `on_submit` builds `MuxLaunchRequest`. `LaunchSpecFormState`, its tests, and the `form.rs` helpers only it used are gone. `widgets::launch_argv` holds the argv helpers both forms use: `DefaultArgv`, the effective argv, option flags, the carry rule, and one quoter (`argv::shell_quote`). The pin edit form steps its harness with the framework's `Choice`. One grammar in `src/argv.rs` (module docs and `docs/operations.md`) splits every argv. `parse_command_line` is strict, for typed argv: the launch form's check, the pin edit save, and `pin edit --set`. `split_command_line` reads text as far as it goes. CSP-777 is covered: both forms decide the default argv when they open and when the harness or cwd changes, and draw from that. The framework is unchanged; `form.md` gained "Keeping fields in step". ADR 0169 supersedes ADR 0097 (the index marks it) and narrows ADR 0096.

Verified: argv tests (quotes, escapes, joined pieces, empty arguments, nothing expanded, unfinished text both ways, `"a\b"`). The launch_argv tests. The mux_launch tests, updated deliberately: `↓` from the name reaches `server`, `Tab` and `↓` walk one ring, failed checks take the focus, drawing twice reads the kept default, and the three insta snapshots take the `server` row. The forms and text_entry consistency tests (one more `↓` to reach cwd, field ids). Two new pin_edit tests. `conspectus tui --snapshot` frames before and after: default, worktree on, socket on and typed, an error, long name and argv, options, an unclosed quote, at 100x30, 60x20, and 40x10. Pin edit frames are unchanged. fmt, clippy (-D warnings), the full isolated suite (4098 passed, 0 failed, 1 ignored), rustdoc (-D warnings), and `git diff --check` pass.

**Regressions**
- `↓`/`Tab` from the name stop on the new `server` row before the harness. The form is a row taller, and reaching a later field takes one more step.
- The socket checkbox moved from the end of the name row to the `server` row.
- With the name and harness both empty, the form reports the name first.
- `←`/`→` inside a text field no longer clear the error line; only an edit does.
- `split_command_line` (previews, history, rebuild, pin overrides) keeps a backslash inside double quotes before an ordinary character (`"a\b"` is `a\b`, was `ab`). It reads a last lone `\` as itself (it was dropped).
- The pin edit form's save and `pin edit --set argv=…` refuse an unclosed quote or a trailing backslash; they used to accept them.
- Toggling an option or stepping the harness on an argv with an unclosed quote writes it back closed. The launch form used to ignore the toggle.
- An argv the forms write back leaves `@`, `%`, and `,` bare and writes a lone `;` as `\;`.

**Consistency wins**
- One ring of tab stops (house rule 6): the socket checkbox is a normal row, and `Tab`/`↓` and `S-Tab`/`↑` visit the same fields.
- The name field gets the whole row; the checkbox used to take 12 cells of it.
- A refused launch moves the cursor to the field to fix (house rule 7).
- One tokenizer for both forms and the previews: the pin edit form's checkboxes and its save read an argv the same way, and the quoting is documented. One quoter for the forms and the previews.
- Harness stepping, option flags, the carry rule, and the default argv live in one place.
- Drawing the launch and pin edit forms reads no files.
<!-- SECTION:FINAL_SUMMARY:END -->
