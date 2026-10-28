---
id: CSP-733
title: Extract `LineEdit` and `TextField`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-723
  - CSP-721
  - CSP-728
  - CSP-715
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
  - docs/adr/0030-tui-text-input-primitive.md
priority: high
type: enhancement
ordinal: 819000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Eleven text-entry surfaces use two edit engines (`tui-input` through `TextInputState`, and the hand-rolled `FindPrompt` in the viewer) and four renderers of one editing state, with three cursor-windowing algorithms; only the viewer's is cell-width aware. The search and picker query lines draw no caret. Three code paths copy the value before a key and compare after to learn whether it changed. Seven match sites handle `InputOutcome::Confirm`, three of them unreachable (inventory slice 03).

Scope: Create `tuikit::text_edit` (or the module name the contribution guide settles on) with `LineEdit`, the only edit engine, wrapping `tui-input` and returning `changed`, `consumed`, and `visual_cursor`; `TextField`, one inline renderer with one cell-width-aware windowing algorithm, optional scroll marks and prefix glyph, a caret cue from the theme, and a placeholder. Handle `Event::Paste` for single-line fields, map ALT to word motion, and have the runtime enable bracketed paste. Replace `TextInputState`, `FindPrompt`, and the form, query-line, and find-prompt renderers.

TUI changes:
- Consistency wins: wide characters edit correctly; search and picker query lines show a caret; paste and word motion work everywhere; one text-editing behavior in all eleven surfaces
- Regressions or behavior changes: the viewer's find prompt gains `tui-input`'s editing keys and loses its own variants where they differ

Tests and checks: Tests for insert, delete, home and end, word motion, kill-line, paste, and wide characters through `LineEdit`. Snapshots of the query lines and the viewer find footer. The behavior-level tests from the text-entry defect task pass unchanged.

Blockers: The text and theme extractions, the key tables, and the text-entry defect task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `LineEdit` is the only text-edit engine; `TextInputState`, `FindPrompt`, and the three cursor-windowing algorithms are replaced by it and `TextField`
- [ ] #2 `LineEdit` reports whether the value changed, and the three copy-and-compare probes are removed
- [ ] #3 Search and picker query lines draw a caret
- [ ] #4 Bracketed paste is handled for single-line fields, ALT word motion works, and editing is cell-width correct
- [ ] #5 The three unreachable `InputOutcome::Confirm` arms in the forms are removed
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
