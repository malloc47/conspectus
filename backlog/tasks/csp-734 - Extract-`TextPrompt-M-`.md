---
id: CSP-734
title: Extract `TextPrompt<M>`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-733
  - CSP-729
  - CSP-726
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
priority: medium
type: enhancement
ordinal: 820000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Five surfaces are the same dialog: rename (session, mux, pin), the worktree branch prompt, and the Controls max-age and contains editors. Each has its own empty-input behavior (a silent no-op in three, a toast in one), pin rename needs a `SlotPrompt` side flag to re-route `CommitRename`, and Esc in the branch prompt returns to the menu while rename closes (inventory slice 03, section 2).

Scope: Create `TextPrompt<M>` with a title, an initial value, and an `on_confirm: Fn(String) -> PromptResult<M>` returning `Commit(M)`, `Reject(String)`, or `Stay`. A rejection shows one error line in the footer slot until the next edit. Migrate the five surfaces; the two rename paths become two closures, removing `SlotPrompt`. Esc steps back one level and cancel is silent.

TUI changes:
- Consistency wins: every rejection explains itself; one prompt for five surfaces; the pin rename prompt no longer needs a routing flag
- Regressions or behavior changes: the prompt modal grows one row while an error shows; the empty-pin-rename toast becomes the prompt's error line

Tests and checks: Tests for each surface's validation and for confirm, reject, cancel, and back. `--snapshot` of each prompt with and without an error.

Blockers: `LineEdit`, the overlay contract, and the modal frame.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `TextPrompt<M>` replaces the rename overlay, the worktree branch prompt, and the Controls max-age and contains editors
- [ ] #2 `PromptResult` is the only way a prompt accepts, rejects, or stays, and a rejection shows an error line until the next edit
- [ ] #3 `SlotPrompt` and the `CommitRename` re-routing are removed
- [ ] #4 Esc steps back one level and cancel produces no status message
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
