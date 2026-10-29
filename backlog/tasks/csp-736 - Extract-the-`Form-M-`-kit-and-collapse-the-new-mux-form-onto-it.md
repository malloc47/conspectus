---
id: CSP-736
title: Extract the `Form<M>` kit and collapse the new mux form onto it
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 06:58'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-734
  - CSP-735
  - CSP-729
  - CSP-727
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: enhancement
ordinal: 822000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three multi-field forms share a row kit in `form.rs` but diverge on Enter (the new mux form advances, the other two submit from any row), Tab stops (mux launch has separate row and stop rings), error display (three styles plus four silent failures), footers (pinned in pin edit, scrolling in the others), the path hint, and labels. The kit paints 17 raw colours and `√` bypasses the glyph table. `warning_row` and `tagged_input_row` have no callers. Radio and checkbox rows both render `[x]` (inventory slice 03, sections 3 to 6).

Scope: Create `Form<M>` with an ordered list of fields and a `FieldKind` enum (Text with a validator, Path, Choice with optional custom entry, Toggle, CheckGroup, OneOf, ReadOnly, Note), per-field visibility, one tab ring, `Enter` submits from any row, an error line in the footer slot, a pinned wrapped footer generated from the form key table, and a label/value grid. Give one-of-N rows a glyph distinct from the checkbox (vetted in `glyphs`). Migrate the new mux form first as the smallest instance. Adopt one label vocabulary (the shorter form).

TUI changes:
- Consistency wins: one Enter, Tab, error, and footer behavior in forms; labels agree; radio rows no longer look like checkboxes
- Regressions or behavior changes: the new mux form no longer advances on Enter from the name field; footers that scrolled away on short terminals are now pinned; labels shorten (`name`, `socket`, `argv`, `branch`)

Tests and checks: Form tests per field kind and per key. `--snapshot` of the new mux form at 80x24 and 40x10 with and without an error. The `consistency_tests/forms.rs` checks pass.

Blockers: `TextPrompt`, `PathField`, the overlay contract, and the selection extraction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `Form<M>` provides the field kinds, one tab ring, submit-from-any-row, an error line, and a pinned generated footer
- [x] #2 The new mux form is an instance of `Form<M>`
- [x] #3 One-of-N rows use a glyph distinct from the checkbox, and the glyph is vetted
- [x] #4 `warning_row`, `tagged_input_row`, and the 17 raw colours are gone
- [x] #5 One label vocabulary applies to the form
- [x] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames of the new mux, mux launch, and pin edit forms (100x30, 80x24, 60x20, 40x10; empty, typed, error) into /tmp/tuifw-baseline/frames-736-before.
2. Glyphs: add the one-of-N marks `(•)`/`( )` (BULLET in parentheses, vetted parts) and the checkbox marks `[x]`/`[ ]`, plus the derived-row corner `└` as a mark, with tests.
3. Keys: add `FormKey::Toggle` (Space in FORM_OPTION_KEYS, same `change` verb) so a check group can move its item cursor with Left/Right and flip with Space; hosts map it like OptionNext.
4. tuikit::form: `Form<M>` (Overlay for any Cx) over `Field`s with `FieldKind` Text, Path, Choice (chips, optional custom entry), Toggle, CheckGroup, OneOf, ReadOnly (Note, Derived, Warning tones); per-field `visible_if` over `Values`; field validators; one ring (Tab/Down next, S-Tab/Up previous, Tab completes a path first); Enter submits from any row through `on_submit -> Submit<M>` (Commit, Reject, Close); one error line in the footer slot cleared by the next edit; `FormBody` with a fixed wrapped footer and rows that scroll with the cursor; the row kit (`Cols`, label/field cells, chips, check/one-of/toggle/static/read-only rows, footer lines).
5. Framework tests per field kind and key, layout at normal and narrow sizes, goldens, and a conformance registration; reference page crates/tuikit/docs/controls/form.md; architecture/README rows.
6. Host: new mux form becomes a `Form<Msg>` (field seeds, the CommitMuxNew mapping, one seeding path, `Enter create`); host form.rs helpers delegate to tuikit::form; delete warning_row; mux launch keeps its kit but gains the fixed footer, the error in the footer slot, the radio glyph, and the shorter labels (name, socket, argv); pin edit keeps its layout over FormBody.
7. Update host tests (new_mux_tests, consistency forms/text_entry, keybinding footers) deliberately; ADR for the Form decisions; operator-review rows; docs/operations.md and CHANGELOG for operator-visible changes.
8. After frames, compare by category; fmt, clippy, full isolated suite, rustdoc, diff check; commit in slices; final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four commits: the option boxes and `FormKey::Toggle` (9df1f61b), `tuikit::form` (2f6ca91d), the host move (1ecf49d2), and the docs (093e919b). The first two commits were checked on their own tree (workspace `cargo check`, host widget/keybinding/consistency tests and tuikit glyph/key/form tests pass).

Frames: /tmp/tuifw-baseline/frames-736-{before,after} (new mux, launch, and pin edit forms at 100x30, 80x24, 60x20, 40x10; opened, typed, cwd focused, Enter with and without a name, empty cwd, edit after an error). Compared with /tmp/tuifw-baseline/compare-736.py. Differences by category: (1) Enter from the name field now creates the session (the form closes; the snapshot skips the launch); (2) refusals moved from a toast to the error line above the footer; (3) the footer verb `Enter next/commit` became `Enter create`, which also shortens the wrapped footer by a row at 40 and 60 columns; (4) the status line no longer shows the hand-written new mux hint; (5) at 40x10 the launch form keeps its keys and shows only its focused row (they scrolled away), and the pin edit form shows its focused row (it showed none) and loses its last footer line; (6) the launch form's labels read `name`, `socket`, `argv`, its pin row reads `(•) don't pin ( ) launch, then pin ( ) pin, don't launch`, and its error line lost the blank line before the keys. Pin edit frames at 60x20 and up are byte-identical.

Removed: `warning_row`, the host `SplitBody` (pin edit uses `FormBody`), `NewMuxFormState`/`NewMuxFormWidget`, the dead `Msg::Toast`. No raw `Color::` remains in the form files (the 17 were already themed by CSP-721; the kit is theme tokens only).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tuikit::form`: `Form<M>` (an `Overlay<M, Cx>` for any context) over `Field`s of `FieldKind` Text (with a check), Path (`PathField`), Choice (chips, optional custom value), Toggle, CheckGroup, OneOf, and ReadOnly (Note, Derived, Warning tones), with `visible_if` conditions over `Values`; one ring (Tab/Down next, S-Tab/Up previous, one stop per field, Tab completes a path first); Enter submits from any field (field checks, then `on_submit -> Submit<M>`: Commit, Reject, Close); one error line directly above the footer, cleared by the next edit; a footer generated from the form's key table that stays in view while only the rows scroll (`FormBody`); and one label grid (`Cols`, 11 to 16 cells) with theme tokens only. The row kit is public. One-of-N options draw `(•)`/`( )` (the vetted bullet in parentheses), distinct from `[x]`/`[ ]`; `Space` is its own `FormKey::Toggle` under the same footer verb. The new mux form is a `Form<Msg>` (seeds, two required checks, the `CommitMuxNew` mapping); the launch and pin edit forms keep their rows but draw them through the kit and `FormBody`. `warning_row`, `SplitBody`, and the dead `Msg::Toast` are gone (`tagged_input_row` went in CSP-733; the raw colors in CSP-721). ADR 0168; control page `crates/tuikit/docs/controls/form.md`.

Verified: tuikit form tests per kind and key, goldens at 64x20 and 40x10, and the conformance suite; host tests updated deliberately (Enter-advance tests now press Down; the launch form's three insta snapshots take the new labels and pin row); `conspectus tui --snapshot` before/after frames at 100x30, 80x24, 60x20, and 40x10 (/tmp/tuifw-baseline/frames-736-*). fmt, clippy (-D warnings), the full isolated suite (4089 passed, 0 failed, 1 ignored), rustdoc (-D warnings), and `git diff --check` pass.

**Regressions**
- The new mux form no longer moves from the name field to the cwd on Enter; Enter creates the session from either field (footer `Enter create`, was `Enter next/commit`).
- Opening the new mux form with `n` no longer shows its keys on the status line (the form shows them).
- At 40x10 the pin edit form loses its last footer line to show its focused row.

**Consistency wins**
- One Enter rule: every form submits from any field (house rule 6).
- New mux refusals (`mux: name is required`, `mux: cwd is required`) are on the form's error line above its keys, with the focus on the field, instead of a toast (house rule 7); the launch form's error line no longer has a blank line before the keys and is cut with `…`.
- The new mux and launch forms keep their keys in view on a short terminal and scroll only their rows (house rule 8); at 40x10 the launch form shows its focused row and keys, and the pin edit form, which showed no row there, shows its focused row.
- The launch form's labels shorten to `name`, `socket`, `argv` (house rule 17).
- The launch form's pin row reads `(•) don't pin ( ) launch, then pin ( ) pin, don't launch`; radio rows no longer look like checkboxes.
<!-- SECTION:FINAL_SUMMARY:END -->
