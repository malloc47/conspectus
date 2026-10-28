---
id: CSP-736
title: Extract the `Form<M>` kit and collapse the new mux form onto it
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
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
- [ ] #1 `Form<M>` provides the field kinds, one tab ring, submit-from-any-row, an error line, and a pinned generated footer
- [ ] #2 The new mux form is an instance of `Form<M>`
- [ ] #3 One-of-N rows use a glyph distinct from the checkbox, and the glyph is vetted
- [ ] #4 `warning_row`, `tagged_input_row`, and the 17 raw colours are gone
- [ ] #5 One label vocabulary applies to the form
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
